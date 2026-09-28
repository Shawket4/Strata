//! A deterministic synthetic vault for the performance suite (PLAN §16.7): by default 10 000
//! markdown files mixing Arabic and English prose, wikilinks between notes, frontmatter
//! relations, people, companies, places (nested), documents, tags, block IDs and Obsidian
//! Tasks lines, laid out like a real vault (§6.1).
//!
//! The same seed always yields the same bytes, so timings are comparable run to run.
//!
//! ```
//! let vault = strata_testkit::SyntheticVault::generate(&strata_testkit::SyntheticConfig {
//!     files: 200,
//!     ..strata_testkit::SyntheticConfig::default()
//! });
//! assert_eq!(vault.files.len(), 200);
//! ```

use std::fmt::Write as _;
use std::path::Path;

/// Shape of the generated vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticConfig {
    /// Total markdown files (entities and the task home included).
    pub files: usize,
    /// RNG seed.
    pub seed: u64,
}

impl Default for SyntheticConfig {
    fn default() -> Self {
        Self {
            files: 10_000,
            seed: 2026,
        }
    }
}

/// One file: vault-relative path and content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticFile {
    /// Vault path, e.g. `notes/topic-3/Note 0042 pricing.md`.
    pub path: String,
    /// File content.
    pub content: String,
}

/// Counts by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SyntheticCounts {
    /// Plain notes.
    pub notes: usize,
    /// People.
    pub people: usize,
    /// Companies.
    pub companies: usize,
    /// Places.
    pub places: usize,
    /// Documents.
    pub documents: usize,
    /// Task lines (open and done).
    pub tasks: usize,
    /// Body wikilinks.
    pub links: usize,
    /// Frontmatter relation entries (`related`, `part-of`, `people`, `companies`, ...).
    pub relations: usize,
}

/// The generated vault.
#[derive(Debug, Clone)]
pub struct SyntheticVault {
    /// Every file, in generation order.
    pub files: Vec<SyntheticFile>,
    /// What was generated.
    pub counts: SyntheticCounts,
    /// Words that occur in the prose (search queries are drawn from them).
    pub vocabulary: Vec<&'static str>,
}

const EN: [&str; 48] = [
    "pricing",
    "invoice",
    "contract",
    "meeting",
    "customer",
    "delivery",
    "quarterly",
    "budget",
    "supplier",
    "renewal",
    "shipment",
    "warehouse",
    "payment",
    "tax",
    "report",
    "hiring",
    "roadmap",
    "feedback",
    "discount",
    "subscription",
    "loyalty",
    "churn",
    "margin",
    "audit",
    "office",
    "safe",
    "passport",
    "licence",
    "deadline",
    "proposal",
    "forecast",
    "inventory",
    "logistics",
    "training",
    "partner",
    "campaign",
    "survey",
    "pilot",
    "migration",
    "backup",
    "security",
    "review",
    "planning",
    "strategy",
    "analysis",
    "summary",
    "retainer",
    "estimate",
];

const AR: [&str; 32] = [
    "فاتورة",
    "عقد",
    "اجتماع",
    "عميل",
    "توصيل",
    "ميزانية",
    "مورد",
    "تجديد",
    "شحنة",
    "مخزن",
    "دفع",
    "ضريبة",
    "تقرير",
    "توظيف",
    "خطة",
    "ملاحظات",
    "خصم",
    "اشتراك",
    "ولاء",
    "هامش",
    "مراجعة",
    "مكتب",
    "خزنة",
    "جواز",
    "رخصة",
    "موعد",
    "عرض",
    "توقعات",
    "مخزون",
    "تدريب",
    "شريك",
    "حملة",
];

const FIRST: [&str; 16] = [
    "Ahmed", "Mona", "Shady", "Laila", "Omar", "Nour", "Karim", "Sara", "Youssef", "Hana", "Tarek",
    "Dina", "Mahmoud", "Rania", "Hossam", "Yasmin",
];
const FIRST_AR: [&str; 16] = [
    "أحمد",
    "منى",
    "شادي",
    "ليلى",
    "عمر",
    "نور",
    "كريم",
    "سارة",
    "يوسف",
    "هنا",
    "طارق",
    "دينا",
    "محمود",
    "رانيا",
    "حسام",
    "ياسمين",
];
const LAST: [&str; 8] = [
    "Samir", "Hassan", "Fahmy", "Nabil", "Adel", "Mansour", "Saleh", "Gamal",
];
const COMPANY: [&str; 12] = [
    "Watanya",
    "Acme",
    "Petrol Arrows",
    "Nile",
    "Delta",
    "Sphinx",
    "Horus",
    "Pyramid",
    "Lotus",
    "Oasis",
    "Cairo",
    "Giza",
];
const COMPANY_SUFFIX: [&str; 4] = ["Trading", "Logistics", "Holdings", "Systems"];

/// `SplitMix64`: tiny, deterministic, platform independent.
#[derive(Debug, Clone)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        let n = u64::try_from(n.max(1)).unwrap_or(u64::MAX);
        usize::try_from(self.next() % n).unwrap_or(0)
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

/// Crockford base32 ULID text from 48-bit time and 80 bits of randomness.
fn ulid(rng: &mut Rng, millis: u64) -> String {
    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let random = (u128::from(rng.next()) << 16) ^ u128::from(rng.next() & 0xFFFF);
    let value = (u128::from(millis & 0xFFFF_FFFF_FFFF) << 80) | (random & ((1u128 << 80) - 1));
    (0..26)
        .rev()
        .map(|i| char::from(ALPHABET[usize::try_from((value >> (i * 5)) & 31).unwrap_or(0)]))
        .collect()
}

fn timestamp(day: usize, minute: usize) -> String {
    let date = chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
        .unwrap_or_default()
        .checked_add_days(chrono::Days::new(u64::try_from(day).unwrap_or(0)))
        .unwrap_or_default();
    format!(
        "{}T{:02}:{:02}:00+03:00",
        date.format("%Y-%m-%d"),
        (minute / 60) % 24,
        minute % 60
    )
}

fn yaml_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| format!("\"[[{i}]]\"")).collect();
    format!("[{}]", quoted.join(", "))
}

impl SyntheticVault {
    /// Generates a vault of `config.files` files.
    #[allow(clippy::too_many_lines)] // one linear recipe reads best in one place
    pub fn generate(config: &SyntheticConfig) -> Self {
        let mut rng = Rng(config.seed);
        let total = config.files.max(10);
        let people_n = (total / 50).max(2);
        let companies_n = (total / 100).max(2);
        let places_n = (total / 200).max(2);
        let documents_n = (total / 100).max(1);
        let notes_n = total - people_n - companies_n - places_n - documents_n - 1;
        let base_ms = 1_735_689_600_000u64; // 2025-01-01
        let mut counts = SyntheticCounts::default();
        let mut files = Vec::with_capacity(total);

        let people: Vec<(String, String)> = (0..people_n)
            .map(|i| {
                let f = i % FIRST.len();
                let name = format!("{} {} {i}", FIRST[f], LAST[(i / FIRST.len()) % LAST.len()]);
                (name, format!("{} {i}", FIRST_AR[f]))
            })
            .collect();
        let companies: Vec<String> = (0..companies_n)
            .map(|i| {
                format!(
                    "{} {} {i}",
                    COMPANY[i % COMPANY.len()],
                    COMPANY_SUFFIX[(i / COMPANY.len()) % COMPANY_SUFFIX.len()]
                )
            })
            .collect();
        let places: Vec<String> = (0..places_n).map(|i| format!("Office {i}")).collect();
        let titles: Vec<String> = (0..notes_n)
            .map(|i| {
                if i % 3 == 0 {
                    format!("ملاحظة {i:05} {}", AR[i % AR.len()])
                } else {
                    format!("Note {i:05} {}", EN[i % EN.len()])
                }
            })
            .collect();

        for (i, (name, alias)) in people.iter().enumerate() {
            let company = &companies[i % companies.len()];
            let content = format!(
                "---\nid: {}\nkind: person\naliases: [{alias}]\ntags: [person]\ncreated: {}\nupdated: {}\nrole: {}\ncompanies: {}\n---\n## Summary\n\n## Notes\n{name} handles {} for {company}.\n",
                ulid(&mut rng, base_ms + u64::try_from(i).unwrap_or(0)),
                timestamp(i % 365, i % 1440),
                timestamp(i % 365, (i + 7) % 1440),
                EN[i % EN.len()],
                yaml_list(std::slice::from_ref(company)),
                EN[(i + 3) % EN.len()],
            );
            counts.people += 1;
            counts.relations += 1;
            files.push(SyntheticFile {
                path: format!("people/{name}.md"),
                content,
            });
        }
        for (i, name) in companies.iter().enumerate() {
            let content = format!(
                "---\nid: {}\nkind: company\naliases: [{}]\ntags: [company]\ncreated: {}\nupdated: {}\nindustry: {}\n---\n## Summary\n\n## Notes\n{name} — {} {}.\n",
                ulid(&mut rng, base_ms + 100_000 + u64::try_from(i).unwrap_or(0)),
                AR[i % AR.len()],
                timestamp(i % 365, i % 1440),
                timestamp(i % 365, (i + 9) % 1440),
                EN[(i + 5) % EN.len()],
                AR[(i + 1) % AR.len()],
                EN[(i + 11) % EN.len()],
            );
            counts.companies += 1;
            files.push(SyntheticFile {
                path: format!("companies/{name}.md"),
                content,
            });
        }
        for (i, name) in places.iter().enumerate() {
            let parent = if i > 0 {
                counts.relations += 1;
                format!("part-of: {}\n", yaml_list(&[places[(i - 1) / 2].clone()]))
            } else {
                String::new()
            };
            let content = format!(
                "---\nid: {}\nkind: place\naliases: [مكتب {i}]\ncreated: {}\nupdated: {}\n{parent}---\n## Notes\n",
                ulid(&mut rng, base_ms + 200_000 + u64::try_from(i).unwrap_or(0)),
                timestamp(i % 365, i % 1440),
                timestamp(i % 365, i % 1440),
            );
            counts.places += 1;
            files.push(SyntheticFile {
                path: format!("places/{name}.md"),
                content,
            });
        }
        for i in 0..documents_n {
            let company = &companies[i % companies.len()];
            let person = &people[i % people.len()].0;
            let content = format!(
                "---\nid: {}\nkind: document\naliases: [عقد {i}]\ndoc-type: {}\ncopy: original\ncompanies: {}\npeople: {}\nstatus: stored\ncreated: {}\nupdated: {}\n---\n## Summary\n\n## Custody\n\n## Notes\nSigned {} copy.\n",
                ulid(&mut rng, base_ms + 300_000 + u64::try_from(i).unwrap_or(0)),
                ["contract", "invoice", "licence", "certificate"][i % 4],
                yaml_list(std::slice::from_ref(company)),
                yaml_list(std::slice::from_ref(person)),
                timestamp(i % 365, i % 1440),
                timestamp(i % 365, i % 1440),
                EN[i % EN.len()],
            );
            counts.documents += 1;
            counts.relations += 2;
            files.push(SyntheticFile {
                path: format!("documents/Document {i:04}.md"),
                content,
            });
        }
        let mut home = String::from("# Tasks\n\n## September 2026\n");
        for i in 0..notes_n {
            let mut fm = format!(
                "---\nid: {}\ntags: [{}, {}]\ncreated: {}\nupdated: {}\n",
                ulid(
                    &mut rng,
                    base_ms + 1_000_000 + u64::try_from(i).unwrap_or(0) * 60_000
                ),
                EN[rng.below(EN.len())],
                ["pos", "ops", "finance", "sales", "hr"][i % 5],
                timestamp(i % 600, i % 1440),
                timestamp(i % 600, (i + 30) % 1440),
            );
            if i > 0 && rng.chance(60) {
                let n = 1 + rng.below(3);
                let targets: Vec<String> = (0..n).map(|_| titles[rng.below(i)].clone()).collect();
                counts.relations += n;
                let _ = writeln!(fm, "related: {}", yaml_list(&targets));
            }
            if i > 10 && rng.chance(25) {
                counts.relations += 1;
                let _ = writeln!(fm, "part-of: {}", yaml_list(&[titles[i % 10].clone()]));
            }
            if rng.chance(40) {
                counts.relations += 1;
                let p = people[rng.below(people.len())].0.clone();
                let _ = writeln!(fm, "people: {}", yaml_list(&[p]));
            }
            if rng.chance(30) {
                counts.relations += 1;
                let c = companies[rng.below(companies.len())].clone();
                let _ = writeln!(fm, "companies: {}", yaml_list(&[c]));
            }
            fm.push_str("---\n");
            let mut body = format!("# {}\n\n", titles[i]);
            for p in 0..(2 + rng.below(4)) {
                let mut sentence = Vec::new();
                for _ in 0..(8 + rng.below(20)) {
                    sentence.push(if rng.chance(35) {
                        rng.pick(&AR)
                    } else {
                        rng.pick(&EN)
                    });
                }
                body.push_str(&sentence.join(" "));
                if i > 0 && rng.chance(50) {
                    counts.links += 1;
                    let _ = write!(body, " [[{}]]", titles[rng.below(i)]);
                }
                if rng.chance(20) {
                    counts.links += 1;
                    let _ = write!(body, " [[{}]]", people[rng.below(people.len())].0);
                }
                if rng.chance(15) {
                    let _ = write!(body, " #{}", EN[rng.below(EN.len())]);
                }
                if p == 0 && rng.chance(30) {
                    let _ = write!(body, " ^b{i}");
                }
                body.push_str("\n\n");
            }
            if rng.chance(15) {
                counts.tasks += 1;
                let done = rng.chance(30);
                let _ = writeln!(
                    body,
                    "- [{}] {} {} 📅 2026-{:02}-{:02}{} ^t-{}",
                    if done { "x" } else { " " },
                    rng.pick(&EN),
                    rng.pick(&AR),
                    1 + rng.below(12),
                    1 + rng.below(28),
                    if done { " ✅ 2026-09-01" } else { "" },
                    ulid(&mut rng, base_ms).to_lowercase(),
                );
            }
            if rng.chance(5) {
                counts.tasks += 1;
                let _ = writeln!(
                    home,
                    "- [ ] Invoice {} 🔁 every month on the 1st 📅 2026-10-01 ^t-{}",
                    companies[rng.below(companies.len())],
                    ulid(&mut rng, base_ms).to_lowercase(),
                );
            }
            counts.notes += 1;
            files.push(SyntheticFile {
                path: format!("notes/topic-{}/{}.md", i % 20, titles[i]),
                content: fm + &body,
            });
        }
        files.push(SyntheticFile {
            path: "tasks/Tasks.md".to_owned(),
            content: format!(
                "---\nid: {}\ncreated: {}\nupdated: {}\n---\n{home}",
                ulid(&mut rng, base_ms + 400_000),
                timestamp(0, 0),
                timestamp(0, 0),
            ),
        });
        let mut vocabulary: Vec<&'static str> = EN.iter().chain(AR.iter()).copied().collect();
        vocabulary.sort_unstable();
        Self {
            files,
            counts,
            vocabulary,
        }
    }

    /// Writes every file under `dir` (creating folders).
    pub fn write_to(&self, dir: &Path) -> std::io::Result<()> {
        for f in &self.files {
            let path = dir.join(&f.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &f.content)?;
        }
        Ok(())
    }

    /// Total bytes of all files.
    pub fn total_bytes(&self) -> usize {
        self.files.iter().map(|f| f.content.len()).sum()
    }

    /// `n` search queries drawn deterministically from the vocabulary (one or two words).
    pub fn queries(&self, n: usize, seed: u64) -> Vec<String> {
        let mut rng = Rng(seed);
        (0..n)
            .map(|_| {
                let a = self.vocabulary[rng.below(self.vocabulary.len())];
                if rng.chance(30) {
                    format!("{a} {}", self.vocabulary[rng.below(self.vocabulary.len())])
                } else {
                    a.to_owned()
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_and_sized() {
        let config = SyntheticConfig {
            files: 500,
            seed: 7,
        };
        let a = SyntheticVault::generate(&config);
        let b = SyntheticVault::generate(&config);
        assert_eq!(a.files, b.files);
        assert_eq!(a.files.len(), 500);
        assert_eq!(
            (
                a.counts.people,
                a.counts.companies,
                a.counts.places,
                a.counts.documents
            ),
            (10, 5, 2, 5)
        );
        assert_eq!(a.counts.notes, 500 - 10 - 5 - 2 - 5 - 1);
        let paths: std::collections::BTreeSet<&str> =
            a.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths.len(), 500, "paths are unique");
        let c = SyntheticVault::generate(&SyntheticConfig { seed: 8, ..config });
        assert_ne!(a.files, c.files);
    }

    #[test]
    fn ulids_are_valid_crockford_and_ordered_by_time() {
        let mut rng = Rng(1);
        let a = ulid(&mut rng, 1);
        let b = ulid(&mut rng, 2);
        assert_eq!(a.len(), 26);
        assert!(
            a.chars()
                .all(|c| "0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c))
        );
        assert!(a < b);
    }

    #[test]
    fn prose_mixes_scripts_and_links() {
        let v = SyntheticVault::generate(&SyntheticConfig {
            files: 300,
            seed: 1,
        });
        let notes: Vec<&SyntheticFile> = v
            .files
            .iter()
            .filter(|f| f.path.starts_with("notes/"))
            .collect();
        assert!(notes.iter().any(|f| f.content.contains("[[")));
        assert!(notes.iter().any(|f| {
            f.content
                .chars()
                .any(|c| ('\u{0600}'..='\u{06FF}').contains(&c))
        }));
        assert!(v.counts.tasks > 0 && v.counts.links > 0 && v.counts.relations > 0);
        assert_eq!(v.queries(5, 3), v.queries(5, 3));
    }
}
