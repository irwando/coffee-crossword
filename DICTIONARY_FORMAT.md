# Coffee Crossword Dictionary Format

This document specifies the open dictionary format used by Coffee Crossword. All dictionary files are plain UTF-8 text — no binary encoding, no proprietary format.

---

## Overview

A dictionary file is a plain text file, one entry per line. The filename (without extension) is used as the display name of the dictionary (e.g. `english.txt` → "english").

Entries can be:
- A bare word (most common)
- A word with an annotation (short inline note)
- A word with a full definition
- A word with a variant relationship

Blank lines and lines beginning with `#` are ignored (comments).

---

## Entry formats

### 1. Bare word

```
canter
nectar
recant
```

The simplest and most common format. One word per line, no metadata.

---

### 2. Word with annotation

```
word+ annotation text
```

The `+` separator attaches a short inline note to the word. Annotations are shown next to the word in results without opening a definition window.

**Examples:**

```
esp+ abbr. extrasensory perception
OK+ also okay
```

Annotations should be brief — a few words at most. For longer explanations, use a definition (see below).

---

### 3. Word with definition

```
word|Definition text here.
```

The `|` separator attaches a full definition. Definitions are displayed in the definition window when a word is looked up. They may contain multiple sentences and can be as long as needed.

**Examples:**

```
canter|An easy gallop; also, to move at such a pace.
trance|A half-conscious state, as between sleeping and waking.
```

---

### 4. Word with variant

```
word:canonical_form
```

The `:` separator declares that this entry is a variant of another headword. Variants are grouped with their canonical form in results rather than shown separately.

**Examples:**

```
escargot's:escargot
don't:dont
```

> **Note:** Normalization (stripping punctuation) handles most variant grouping automatically. Explicit variant declarations are only needed when the relationship is non-obvious or when the normalized forms would otherwise differ.

---

### 5. Word with annotation and definition

Annotation and definition may be combined:

```
word+ annotation|Definition text here.
```

**Example:**

```
OK+ also okay|Used to express agreement, acceptance, or approval.
```

---

## Combining formats

The separator characters have a defined parsing order:

1. `:` — variant declaration (processed first; remainder is the canonical form)
2. `+` — annotation separator
3. `|` — definition separator

A line may contain at most one of each separator. Combining `:` with `+` or `|` is not supported — variant entries carry no annotation or definition of their own.

---

## Comments and blank lines

Lines beginning with `#` are comments and are ignored entirely:

```
# This is a comment
# Source: SCOWL 2023, size 70
canter
```

Blank lines are also ignored and may be used to visually separate sections.

---

## Character encoding

All files must be UTF-8 encoded. Unicode letters are fully supported in headwords — accented characters, ligatures, and non-ASCII scripts are all valid.

Digits are treated as letters for matching and length purposes (e.g. `catch-22` normalizes to `catch22`, length 7).

---

## Normalization and matching

When normalization is enabled (the default), the search engine strips all non-letter, non-digit characters from a word before matching and length calculation. This means:

- `don't` and `dont` match the same patterns
- `catch-22` is treated as 7 characters

When normalization is disabled, all characters count literally. Each dictionary entry is matched and measured as written.

---

## File naming

Dictionary files should use lowercase kebab-case names with a `.txt` extension:

```
english.txt
english-proper-nouns.txt
french.txt
cryptic-indicators.txt
```

The filename without extension becomes the dictionary's default display name. Users may rename dictionaries in the app UI; the display name is stored separately from the filename.

---

## Example file

```
# Coffee Crossword dictionary — minimal example
# Source: handcrafted

canter
nectar
recant
trance

# With annotations
OK+ also okay
esp+ abbr. extrasensory perception

# With definitions
canter|An easy gallop; also, to move at such a pace.
trance|A half-conscious state, as between sleeping and waking.

# Variants
escargot's:escargot
don't:dont
```

---

## Relationship to TEA's TSD format

TEA (The TA Crossword Helper by Ross Beresford) used a proprietary binary format called TSD. Two variants existed:

- **TSD1** — XOR-encoded text; the encoding has been reversed and the source format is documented above (the `+`, `|`, and `:` separators originate from TSD1 source files).
- **TSD0** — DAWG-compressed; reverse engineering deferred. Not required for current functionality.

Coffee Crossword uses plain UTF-8 as its native format. This enables version control, community contributions, and external tooling without any proprietary dependencies.

---

## Future extensions

The following are reserved for future use and not yet implemented:

- **Part-of-speech tags** — e.g. `word [n]` or `word [v]`
- **Etymology** — multi-line block format, TBD
- **Frequency score** — for ranking results within a list
- **Multi-word entries** — phrases and compound expressions

Reserved separator characters that should not be used in annotations or definitions until specified: `[`, `]`, `{`, `}`, `<`, `>`.
