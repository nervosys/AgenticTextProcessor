# Semantic Search

ATP includes a built-in TF-IDF semantic search engine for finding contextually similar content without exact pattern matching.

## How It Works

The semantic search uses **Term Frequency–Inverse Document Frequency (TF-IDF)** with cosine similarity to rank documents by relevance to a query.

1. **Tokenization**: Text is split into lowercase tokens, with stop words removed
2. **TF-IDF Indexing**: Each document's term frequency is weighted by inverse document frequency
3. **Cosine Similarity**: Query terms are compared against indexed documents using cosine similarity

## API Usage (Rust)

```rust
use atp_core::semantic::TfIdfIndex;

let mut index = TfIdfIndex::new();

// Index documents
index.add_document("file.rs", 1, "Rust is a systems programming language");
index.add_document("file.rs", 2, "It focuses on safety and performance");
index.add_document("other.py", 1, "Python is a scripting language");

// Finalize the index (computes IDF weights)
index.finalize();

// Query
let results = index.query("systems programming safety", 10);
for doc in &results {
    println!("{} (line {}): score={:.3}", doc.file, doc.line, doc.score);
}
```

## Stop Words

The default stop word list includes 90+ common English words (the, is, at, which, etc.). Custom stop words can be provided:

```rust
let custom_stops: HashSet<String> = ["custom", "words"].iter().map(|s| s.to_string()).collect();
let index = TfIdfIndex::with_stop_words(custom_stops);
```

## WASM

Semantic search is available in the browser via the WASM binding:

```javascript
import { semantic_search } from 'atp-wasm';

const docs = JSON.stringify([
  { file: "a.rs", line: 1, content: "Rust systems programming" },
  { file: "b.py", line: 1, content: "Python scripting" }
]);

const results = semantic_search("systems programming", docs, 5);
console.log(JSON.parse(results));
```
