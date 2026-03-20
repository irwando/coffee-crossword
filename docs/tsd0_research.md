# TSD0 Format — Reverse Engineering Notes

## Status
Partially cracked. Individual word lookup works. Full extraction blocked by DAWG cycle handling.

---

## File Structure

| Offset | Content |
|--------|---------|
| 0x0000 | Magic: `TSD0` (4 bytes) |
| 0x0004 | Max word length: `24` (4 bytes LE) |
| 0x0008 | Header table (unclear — values too large to be file offsets) |
| 0x0070 | Second header table (possible word counts per length?) |
| 0x00C6 | Description string, null-terminated, unencoded ASCII |
| 0x0116 | Index/trie structure begins |
| 0x1000 | **DAWG root node** |
| 0x1000–0xD000 | DAWG node table (0x80/0x00 dominated region) |
| 0xD000–EOF | Secondary data (different structure, not yet decoded) |

---

## DAWG Structure (confirmed working)

The word data is stored as a **DAWG (Directed Acyclic Word Graph)** —
a compressed trie where common suffixes share nodes.

### Node format
- Each node is **26 bytes** — one byte per letter a–z
- Byte at position `i` corresponds to letter `chr(ord('a') + i)`
- Root node is at file offset **0x1000**

### Byte values
| Value | Meaning |
|-------|---------|
| `0x00` | This letter is not a valid continuation at this node |
| `0x80` | Terminal — this letter ends a valid word; no further children |
| Negative (0x81–0xFF, i.e. signed -127 to -1) | Has children AND this is a word end |
| Positive (0x01–0x7F) | Has children; NOT a word end here |

### Navigation
Given current node at address `A` and next letter `ch` (index `i`):
```python
val = data[A + i]
signed = val if val < 128 else val - 256   # interpret as signed byte
next_node = A + signed * 26                 # signed row offset
```

### Word lookup (verified working)
```python
ROOT = 0x1000
ROW_SIZE = 26

def is_valid_word(data, word):
    current = ROOT
    for i, ch in enumerate(word.lower()):
        idx = ord(ch) - ord('a')
        addr = current + idx
        val = data[addr]
        signed = val if val < 128 else val - 256
        is_last = (i == len(word) - 1)
        
        if val == 0x00:
            return False   # dead end
        if val == 0x80:
            return True    # terminal match
        if is_last:
            return signed < 0   # negative = word ends here
        
        current = current + signed * ROW_SIZE
    return False
```

### Verified word lookups
| Word | Result | End byte |
|------|--------|----------|
| `abandon` | ✓ FOUND | `0x80` terminal |
| `abandons` | ✓ FOUND | `0x80` terminal |
| `aardvark` | ✓ FOUND | `0xFF` (-1) negative |
| `aardvarks` | ✓ FOUND | `0xFB` (-5) negative |
| `cat` | ✓ FOUND | `0xFF` (-1) negative |
| `cats` | ✓ FOUND | `0xFD` (-3) negative |
| `the` | ✓ FOUND | `0xFF` (-1) negative |
| `there` | ✓ FOUND | `0xFF` (-1) negative |
| `xyzzy` | ✗ NOT FOUND | dead end |
| `abcde` | ✗ NOT FOUND | positive (not word end) |

---

## Extraction Problem

A simple DFS from the root produces **cycles** because the DAWG shares suffix
nodes across different word paths. The `visited` set prevents infinite loops but
also prevents legitimate re-traversal of shared nodes via different prefixes.

### Why it's hard
In a trie, each node has exactly one parent. In a DAWG, a node can be reached
from many parents — e.g. the suffix `-tion` is shared by hundreds of words.
A simple visited-set DFS will extract some words but miss all others that share
the same suffix nodes.

### Correct approach
Track the **path** (prefix built so far) rather than just visited nodes.
Do NOT add nodes to a global visited set — instead, limit depth to prevent
infinite loops via cycles (max word length = 24, so max depth = 24).

```python
def extract_all(data, node_addr, prefix, results, depth=0):
    if depth > 24:
        return
    for i in range(26):
        ch = chr(ord('a') + i)
        addr = node_addr + i
        val = data[addr]
        if val == 0x00:
            continue
        signed = val if val < 128 else val - 256
        word = prefix + ch
        if val == 0x80:
            results.append(word)   # terminal
        else:
            if signed < 0:
                results.append(word)   # word end + has children
            next_node = node_addr + signed * 26
            # Guard: don't revisit the exact same (node, depth) combination
            extract_all(data, next_node, word, results, depth + 1)
```

**WARNING**: This may still loop if the DAWG has back-edges (cycles pointing
to ancestor nodes). Need to detect and break cycles properly.

---

## Next Steps

1. Implement proper cycle detection using a path-based visited set
   `(node_addr, depth)` rather than just `node_addr`
2. Or: use the header word-count-per-length data to know when to stop
3. Test extraction against known word counts (~67,000 for Core English)
4. Once extraction works, extend to all 28 TSD files

---

## Files
- `docs/tea-original-help/Core_English.tsd` — the file being analysed
- `docs/tea-original-help/tsbuild1.tsd` — TSD1 format (fully cracked, XOR 0xBD)
- `docs/tea-original-help/tsbuild2.tsd` — TSD1 format (fully cracked, XOR 0xBD)
