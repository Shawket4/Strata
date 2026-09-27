"""Regenerates granite_quint8_reference.json: reference embeddings computed with the Python
onnxruntime + tokenizers packages (no padding, CLS pooling, L2 normalisation), against which
tests/onnx_real.rs checks the Rust pipeline.

    pip install onnxruntime==1.30.0 tokenizers numpy
    python reference.py /opt/models/granite-embedding-97m-multilingual-r2 > granite_quint8_reference.json
"""
import json
import sys

import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

TEXTS = [
    "Watanya's contract is in the safe at the Nasr City office.",
    "عقد وطنية في الخزنة في مكتب مدينة نصر",
    "العقد بتاع Watanya في الـ safe في مكتب مدينة نصر",
    "Banana bread recipe with walnuts and cinnamon.",
]

model_dir = sys.argv[1]
tok = Tokenizer.from_file(f"{model_dir}/tokenizer.json")
tok.no_padding()
tok.no_truncation()
opts = ort.SessionOptions()
opts.intra_op_num_threads = 1
opts.inter_op_num_threads = 1
session = ort.InferenceSession(f"{model_dir}/onnx/model_quint8_avx2.onnx", opts, providers=["CPUExecutionProvider"])
rows = []
for text in TEXTS:
    enc = tok.encode(text)
    ids = np.array([enc.ids], dtype=np.int64)
    mask = np.array([enc.attention_mask], dtype=np.int64)
    cls = session.run(None, {"input_ids": ids, "attention_mask": mask})[0][0, 0]
    rows.append({"text": text, "ids": enc.ids, "vector": [round(float(x), 7) for x in cls / np.linalg.norm(cls)]})
json.dump(rows, sys.stdout, ensure_ascii=False)
print()
