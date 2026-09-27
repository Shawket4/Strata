//! Model-independent embedding steps: batching under a token budget, padding, pooling, and L2
//! normalisation. Pure functions, unit-tested without a model.

use std::ops::Range;

use super::{EmbedError, Pooling};

/// Splits sequences (given their token lengths, in order) into consecutive batches whose padded
/// size (`len(batch) × longest`) stays within `max_batch_tokens`. A sequence longer than the
/// budget gets a batch of its own.
pub fn plan_batches(lengths: &[usize], max_batch_tokens: usize) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut longest = 0;
    for (i, &len) in lengths.iter().enumerate() {
        let new_longest = longest.max(len);
        let count = i - start + 1;
        if i > start && new_longest.saturating_mul(count) > max_batch_tokens {
            out.push(start..i);
            start = i;
            longest = len;
        } else {
            longest = new_longest;
        }
    }
    if start < lengths.len() {
        out.push(start..lengths.len());
    }
    out
}

/// A padded batch in row-major `[batch, seq]` layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaddedBatch {
    /// Token IDs.
    pub input_ids: Vec<i64>,
    /// 1 for real tokens, 0 for padding.
    pub attention_mask: Vec<i64>,
    /// Rows.
    pub batch: usize,
    /// Columns (longest sequence).
    pub seq: usize,
}

/// Right-pads `seqs` with `pad_id`.
pub fn pad(seqs: &[Vec<u32>], pad_id: u32) -> PaddedBatch {
    let seq = seqs.iter().map(Vec::len).max().unwrap_or(0);
    let mut input_ids = Vec::with_capacity(seqs.len() * seq);
    let mut attention_mask = Vec::with_capacity(seqs.len() * seq);
    for s in seqs {
        input_ids.extend(s.iter().map(|&t| i64::from(t)));
        attention_mask.extend(std::iter::repeat_n(1, s.len()));
        input_ids.extend(std::iter::repeat_n(i64::from(pad_id), seq - s.len()));
        attention_mask.extend(std::iter::repeat_n(0, seq - s.len()));
    }
    PaddedBatch {
        input_ids,
        attention_mask,
        batch: seqs.len(),
        seq,
    }
}

/// Pools `hidden` (`[batch, seq, dim]`, row-major) into one vector per row.
pub fn pool(
    hidden: &[f32],
    mask: &[i64],
    batch: usize,
    seq: usize,
    dim: usize,
    pooling: Pooling,
) -> Result<Vec<Vec<f32>>, EmbedError> {
    if hidden.len() != batch * seq * dim || mask.len() != batch * seq {
        return Err(EmbedError::Output(format!(
            "hidden has {} values and mask {}, expected {}×{}×{}",
            hidden.len(),
            mask.len(),
            batch,
            seq,
            dim
        )));
    }
    let mut out = Vec::with_capacity(batch);
    for b in 0..batch {
        let row = &hidden[b * seq * dim..(b + 1) * seq * dim];
        let v = match pooling {
            Pooling::Cls => row[..dim].to_vec(),
            Pooling::Mean => {
                let mut acc = vec![0f32; dim];
                let mut n = 0f32;
                for t in 0..seq {
                    if mask[b * seq + t] != 0 {
                        n += 1.0;
                        for (a, x) in acc.iter_mut().zip(&row[t * dim..(t + 1) * dim]) {
                            *a += x;
                        }
                    }
                }
                if n > 0.0 {
                    acc.iter_mut().for_each(|a| *a /= n);
                }
                acc
            }
        };
        out.push(v);
    }
    Ok(out)
}

/// Scales `v` to unit length (a zero vector stays zero).
pub fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        v.iter_mut().for_each(|x| *x /= norm);
    }
}

/// Cosine similarity of two unit vectors (their dot product).
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn batches_respect_the_padded_token_budget_and_keep_order() {
        assert_eq!(plan_batches(&[], 10), Vec::<Range<usize>>::new());
        assert_eq!(plan_batches(&[3, 3, 3, 3], 10), vec![0..3, 3..4]);
        // A long sequence raises the padded size of everything batched with it.
        assert_eq!(plan_batches(&[2, 2, 8, 2], 10), vec![0..2, 2..3, 3..4]);
        // Longer than the budget: alone.
        assert_eq!(plan_batches(&[20, 1, 1], 10), vec![0..1, 1..3]);
    }

    #[test]
    fn pad_right_pads_ids_and_masks() {
        let b = pad(&[vec![5, 6, 7], vec![9]], 0);
        assert_eq!(
            b,
            PaddedBatch {
                input_ids: vec![5, 6, 7, 9, 0, 0],
                attention_mask: vec![1, 1, 1, 1, 0, 0],
                batch: 2,
                seq: 3
            }
        );
    }

    #[test]
    fn cls_takes_the_first_token_and_mean_ignores_padding() {
        // batch 2, seq 3, dim 2
        let hidden = [
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, // row 0: all attended
            7.0, 8.0, 9.0, 10.0, 100.0, 100.0, // row 1: last token is padding
        ];
        let mask = [1, 1, 1, 1, 1, 0];
        assert_eq!(
            pool(&hidden, &mask, 2, 3, 2, Pooling::Cls).expect("shape"),
            vec![vec![1.0, 2.0], vec![7.0, 8.0]]
        );
        assert_eq!(
            pool(&hidden, &mask, 2, 3, 2, Pooling::Mean).expect("shape"),
            vec![vec![3.0, 4.0], vec![8.0, 9.0]]
        );
        assert_eq!(
            pool(&hidden, &mask, 2, 3, 3, Pooling::Cls),
            Err(EmbedError::Output(
                "hidden has 12 values and mask 6, expected 2×3×3".into()
            ))
        );
    }

    #[test]
    fn l2_normalisation_gives_unit_vectors() {
        let mut v = vec![3.0, 4.0];
        l2_normalize(&mut v);
        assert_eq!(v, vec![0.6, 0.8]);
        let mut z = vec![0.0, 0.0];
        l2_normalize(&mut z);
        assert_eq!(z, vec![0.0, 0.0]);
        assert!((dot(&v, &v) - 1.0).abs() < 1e-6);
    }
}
