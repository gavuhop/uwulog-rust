#!/usr/bin/env python3
"""
ast_index.py — Token-Optimized Rust Tree, AST Skeleton, Symbol & Hotspot Extractor
for uwulog-rust (zero external dependencies, Python 3.8+ stdlib only).

Subcommands:
  tree      Workspace/crate directory map with Tier labels, LOC, and top-level symbols
  skeleton  Extract Rust structs/enums/traits/impls/fn signatures + [Lstart-Lend], omitting bodies
  symbol    Locate definitions and cross-crate call/reference sites with enclosing fn context
  hotspots  Identify largest functions/files, locks (RwLock/Mutex/Atomic), par_iter, unsafe
  fields    Inspect SSOT StandardField canonical names, aliases, FieldType & Arrow '__' columns
  keymap    Inspect KeyAction variants, KeyContext hierarchy & default shortcuts in core_keymap
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
import re
import sys
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

# ---------------------------------------------------------------------------
# Workspace & 5-Tier Architecture Metadata
# ---------------------------------------------------------------------------

CRATE_TIERS: Dict[str, Tuple[int, str]] = {
    "uwu-core-util": (0, "Tier 0: Foundations"),
    "uwu-core-schema": (0, "Tier 0: Foundations"),
    "uwu-core-protocol": (0, "Tier 0: Foundations"),
    "uwu-core-filter": (0, "Tier 0: Foundations"),
    "uwu-icons": (0, "Tier 0: Foundations"),
    "uwu-driver-transport": (1, "Tier 1: Drivers & Transport"),
    "uwu-driver-sources": (1, "Tier 1: Drivers & Transport"),
    "uwu-core-engine": (2, "Tier 2: Core Data Engine"),
    "uwu-core-workspace": (3, "Tier 3: Workspace & Keymap"),
    "uwu-core-keymap": (3, "Tier 3: Workspace & Keymap"),
    "uwu-cli": (4, "Tier 4: Presentation & Binaries"),
    "uwu-ui-gui": (4, "Tier 4: Presentation & Binaries"),
    "uwu-cli-agent": (4, "Tier 4: Presentation & Binaries"),
    "uwu-benchmarks": (4, "Tier 4: Presentation & Binaries"),
}

IGNORED_DIRS = {"target", ".git", "dist", "node_modules", "__pycache__", ".idea"}


def find_repo_root(start: Optional[Path] = None) -> Path:
    cur = (start or Path.cwd()).resolve()
    for candidate in [cur, *cur.parents]:
        if (candidate / "Cargo.toml").is_file() and (candidate / "crates").is_dir():
            return candidate
    return Path(__file__).resolve().parent.parent.parent


@dataclasses.dataclass
class CrateInfo:
    name: str
    rel_dir: str
    abs_dir: Path
    tier: int
    tier_label: str
    internal_deps: List[str]


def discover_crates(repo_root: Path) -> List[CrateInfo]:
    root_cargo = repo_root / "Cargo.toml"
    members: List[str] = []
    if root_cargo.is_file():
        text = root_cargo.read_text(encoding="utf-8", errors="replace")
        m = re.search(r"members\s*=\s*\[(.*?)\]", text, re.S)
        if m:
            members = re.findall(r'"([^"]+)"', m.group(1))
    if not members:
        members = [
            str(p.parent.relative_to(repo_root))
            for p in sorted((repo_root / "crates").glob("*/Cargo.toml"))
        ]

    crates: List[CrateInfo] = []
    for rel in members:
        cdir = repo_root / rel
        ctoml = cdir / "Cargo.toml"
        if not ctoml.is_file():
            continue
        ctext = ctoml.read_text(encoding="utf-8", errors="replace")
        pkg_match = re.search(r'(?m)^\s*name\s*=\s*"([^"]+)"', ctext)
        pkg_name = pkg_match.group(1) if pkg_match else Path(rel).name
        tier, tier_label = CRATE_TIERS.get(pkg_name, (-1, "Unknown Tier"))

        deps: List[str] = []
        in_deps = False
        for line in ctext.splitlines():
            s = line.strip()
            if s.startswith("["):
                tbl_dep = re.match(r"^\[(?:target\.[^\]]+\.)?dependencies\.(uwu-[a-zA-Z0-9_-]+)\]$", s)
                if tbl_dep:
                    deps.append(tbl_dep.group(1))
                    in_deps = False
                else:
                    in_deps = bool(re.match(r"^\[(?:(?:workspace|target\.[^\]]+)\.)?dependencies\]$", s))
                continue
            if in_deps and not s.startswith("#") and "=" in s:
                dep_key = s.split("=", 1)[0].strip().split(".")[0]
                if dep_key.startswith("uwu-"):
                    deps.append(dep_key)

        crates.append(
            CrateInfo(
                name=pkg_name,
                rel_dir=rel.replace("\\", "/"),
                abs_dir=cdir,
                tier=tier,
                tier_label=tier_label,
                internal_deps=sorted(set(deps)),
            )
        )
    return crates


# ---------------------------------------------------------------------------
# Rust-Aware Lexer (Handles Nested Block Comments, Raw Strings, Lifetimes)
# ---------------------------------------------------------------------------

@dataclasses.dataclass
class Token:
    kind: str  # IDENT, NUMBER, STRING, CHAR, COMMENT_LINE, DOC_COMMENT, COMMENT_BLOCK, PUNCT
    text: str
    line: int
    end_line: int
    start: int
    end: int


def tokenize_rust(source: str) -> List[Token]:
    tokens: List[Token] = []
    n = len(source)
    i = 0
    line = 1

    while i < n:
        ch = source[i]

        # Whitespace
        if ch.isspace():
            if ch == "\n":
                line += 1
            i += 1
            continue

        # Line comment or doc comment
        if ch == "/" and i + 1 < n and source[i + 1] == "/":
            start = i
            start_line = line
            j = source.find("\n", i)
            if j == -1:
                j = n
            text = source[start:j]
            is_doc = text.startswith("///") and not text.startswith("////")
            is_inner_doc = text.startswith("//!")
            kind = "DOC_COMMENT" if (is_doc or is_inner_doc) else "COMMENT_LINE"
            tokens.append(Token(kind, text, start_line, start_line, start, j))
            i = j
            continue

        # Block comment (Rust supports nested /* /* ... */ */)
        if ch == "/" and i + 1 < n and source[i + 1] == "*":
            start = i
            start_line = line
            depth = 1
            i += 2
            while i < n and depth > 0:
                if source[i] == "\n":
                    line += 1
                    i += 1
                elif source[i] == "/" and i + 1 < n and source[i + 1] == "*":
                    depth += 1
                    i += 2
                elif source[i] == "*" and i + 1 < n and source[i + 1] == "/":
                    depth -= 1
                    i += 2
                else:
                    i += 1
            text = source[start:i]
            is_doc = (text.startswith("/**") and not text.startswith("/***")) or text.startswith("/*!")
            kind = "DOC_COMMENT" if is_doc else "COMMENT_BLOCK"
            tokens.append(Token(kind, text, start_line, line, start, i))
            continue

        # Raw strings: r"...", r#"..."#, br#"..."#, cr#"..."#
        raw_prefix_len = 0
        if ch == "r" and i + 1 < n and source[i + 1] in ('"', "#"):
            raw_prefix_len = 1
        elif ch in ("b", "c") and i + 2 < n and source[i + 1] == "r" and source[i + 2] in ('"', "#"):
            raw_prefix_len = 2

        if raw_prefix_len > 0:
            j = i + raw_prefix_len
            hashes = 0
            while j < n and source[j] == "#":
                hashes += 1
                j += 1
            if j < n and source[j] == '"':
                start = i
                start_line = line
                j += 1
                closing = '"' + ("#" * hashes)
                end_pos = source.find(closing, j)
                if end_pos == -1:
                    end_pos = n
                else:
                    end_pos += len(closing)
                line += source[j:end_pos].count("\n")
                tokens.append(Token("STRING", source[start:end_pos], start_line, line, start, end_pos))
                i = end_pos
                continue

        # Normal/byte/c strings: "...", b"...", c"..."
        str_prefix_len = 0
        if ch == '"':
            str_prefix_len = 0
        elif ch in ("b", "c") and i + 1 < n and source[i + 1] == '"':
            str_prefix_len = 1
        else:
            str_prefix_len = -1

        if str_prefix_len >= 0:
            start = i
            start_line = line
            i += str_prefix_len + 1
            while i < n:
                c = source[i]
                if c == "\n":
                    line += 1
                    i += 1
                elif c == "\\":
                    if i + 1 < n and source[i + 1] == "\n":
                        line += 1
                    i += 2
                elif c == '"':
                    i += 1
                    break
                else:
                    i += 1
            tokens.append(Token("STRING", source[start:i], start_line, line, start, i))
            continue

        # Byte char b'x' or Char literal 'x' / '\n' / '\u{...}' vs Lifetime 'a
        if ch == "b" and i + 1 < n and source[i + 1] == "'":
            start = i
            start_line = line
            j = i + 2
            if j < n and source[j] == "\\":
                j += 2
                while j < n and source[j] not in ("'", "\n"):
                    j += 1
            elif j < n:
                j += 1
            if j < n and source[j] == "'":
                j += 1
                tokens.append(Token("CHAR", source[start:j], start_line, line, start, j))
                i = j
                continue

        if ch == "'":
            start = i
            start_line = line
            if i + 1 < n and source[i + 1] == "\\":
                j = i + 2
                if j < n:
                    j += 1
                while j < n and source[j] not in ("'", "\n") and (j - i) < 16:
                    j += 1
                if j < n and source[j] == "'":
                    j += 1
                    tokens.append(Token("CHAR", source[start:j], start_line, line, start, j))
                    i = j
                    continue
            elif i + 2 < n and source[i + 2] == "'" and source[i + 1] not in ("'", "\n", "\\"):
                tokens.append(Token("CHAR", source[start : i + 3], start_line, line, start, i + 3))
                i += 3
                continue
            tokens.append(Token("PUNCT", "'", start_line, start_line, start, start + 1))
            i += 1
            continue

        # Raw identifier r#ident
        if ch == "r" and i + 2 < n and source[i + 1] == "#" and (source[i + 2].isalpha() or source[i + 2] == "_"):
            start = i
            j = i + 2
            while j < n and (source[j].isalnum() or source[j] == "_"):
                j += 1
            tokens.append(Token("IDENT", source[start:j], line, line, start, j))
            i = j
            continue

        # Identifier
        if ch.isalpha() or ch == "_":
            start = i
            j = i + 1
            while j < n and (source[j].isalnum() or source[j] == "_"):
                j += 1
            tokens.append(Token("IDENT", source[start:j], line, line, start, j))
            i = j
            continue

        # Number
        if ch.isdigit():
            start = i
            j = i + 1
            while j < n and (source[j].isalnum() or source[j] in ("_", ".")):
                if source[j] == "." and j + 1 < n and source[j + 1] == ".":
                    break
                j += 1
            tokens.append(Token("NUMBER", source[start:j], line, line, start, j))
            i = j
            continue

        # Punctuation
        tokens.append(Token("PUNCT", ch, line, line, i, i + 1))
        i += 1

    return tokens


# ---------------------------------------------------------------------------
# Structural Rust Skeleton & Symbol Extractor
# ---------------------------------------------------------------------------

@dataclasses.dataclass
class AstItem:
    kind: str  # mod, struct, enum, trait, impl, fn, type, const, static, pub_use, macro
    name: str
    visibility: str
    start_line: int
    end_line: int
    signature: str
    doc: List[str] = dataclasses.field(default_factory=list)
    attrs: List[str] = dataclasses.field(default_factory=list)
    is_test: bool = False
    children: List["AstItem"] = dataclasses.field(default_factory=list)
    body_text: Optional[str] = None  # Used for compact struct/enum fields


def _find_matching(tokens: Sequence[Token], open_idx: int, open_ch: str, close_ch: str) -> int:
    depth = 0
    n = len(tokens)
    for idx in range(open_idx, n):
        t = tokens[idx]
        if t.kind == "PUNCT":
            if t.text == open_ch:
                depth += 1
            elif t.text == close_ch:
                depth -= 1
                if depth == 0:
                    return idx
    return n - 1


def _is_cfg_test_attrs(attrs: Sequence[str]) -> bool:
    for a in attrs:
        compact = re.sub(r"\s+", "", a)
        if "#[cfg(test)]" in compact or "#![cfg(test)]" in compact:
            return True
        if "#[test]" in compact or "tokio::test" in compact:
            return True
        if compact.startswith("#[cfg(") and ("test)" in compact or ",test," in compact or "(test," in compact):
            return True
    return False


def _collapse_ws(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def _extract_doc_summary(doc_tokens: Sequence[Token]) -> List[str]:
    lines: List[str] = []
    for tok in doc_tokens:
        for raw in tok.text.splitlines():
            s = raw.strip()
            if s.startswith("///"):
                s = s[3:].strip()
            elif s.startswith("//!"):
                s = s[3:].strip()
            if s:
                lines.append(s)
    if len(lines) > 2:
        return [lines[0], f"... ({len(lines)} doc lines)"]
    return lines


def _scan_scope(
    tokens: Sequence[Token],
    source: str,
    start_idx: int,
    end_idx: int,
    parent_is_test: bool = False,
) -> List[AstItem]:
    items: List[AstItem] = []
    i = start_idx

    while i <= end_idx:
        # Skip non-code comments unless doc comments
        pending_docs: List[Token] = []
        pending_attrs: List[str] = []

        while i <= end_idx:
            t = tokens[i]
            if t.kind in ("COMMENT_LINE", "COMMENT_BLOCK"):
                i += 1
            elif t.kind == "DOC_COMMENT":
                pending_docs.append(t)
                i += 1
            elif t.kind == "PUNCT" and t.text == "#":
                # Outer attribute #[...] or inner #![...]
                if i + 1 <= end_idx and tokens[i + 1].kind == "PUNCT" and tokens[i + 1].text == "!":
                    if i + 2 <= end_idx and tokens[i + 2].kind == "PUNCT" and tokens[i + 2].text == "[":
                        close_idx = _find_matching(tokens, i + 2, "[", "]")
                        i = close_idx + 1
                        pending_docs.clear()
                        continue
                if i + 1 <= end_idx and tokens[i + 1].kind == "PUNCT" and tokens[i + 1].text == "[":
                    close_idx = _find_matching(tokens, i + 1, "[", "]")
                    attr_txt = source[tokens[i].start : tokens[close_idx].end]
                    pending_attrs.append(_collapse_ws(attr_txt))
                    i = close_idx + 1
                    continue
                break
            else:
                break

        if i > end_idx:
            break

        item_start_idx = i
        item_start_line = tokens[i].line
        item_is_test = parent_is_test or _is_cfg_test_attrs(pending_attrs)

        # Collect visibility + qualifiers before main keyword
        vis = "private"
        qualifiers: List[str] = []
        j = i
        while j <= end_idx:
            tk = tokens[j]
            if tk.kind in ("COMMENT_LINE", "COMMENT_BLOCK", "DOC_COMMENT"):
                j += 1
                continue
            if tk.kind == "IDENT" and tk.text == "pub":
                vis = "pub"
                if j + 1 <= end_idx and tokens[j + 1].kind == "PUNCT" and tokens[j + 1].text == "(":
                    c = _find_matching(tokens, j + 1, "(", ")")
                    vis = _collapse_ws(source[tk.start : tokens[c].end])
                    j = c + 1
                else:
                    j += 1
                continue
            if tk.kind == "IDENT" and tk.text in ("async", "unsafe", "default"):
                qualifiers.append(tk.text)
                j += 1
                continue
            if tk.kind == "IDENT" and tk.text == "const":
                # Could be `const fn` or `const NAME: T = ...`
                k = j + 1
                while k <= end_idx and tokens[k].kind in ("COMMENT_LINE", "COMMENT_BLOCK"):
                    k += 1
                if k <= end_idx and tokens[k].kind == "IDENT" and tokens[k].text in ("fn", "unsafe", "extern"):
                    qualifiers.append("const")
                    j += 1
                    continue
                break
            if tk.kind == "IDENT" and tk.text == "extern":
                qualifiers.append("extern")
                j += 1
                if j <= end_idx and tokens[j].kind == "STRING":
                    qualifiers.append(tokens[j].text)
                    j += 1
                continue
            break

        if j > end_idx:
            break

        kw_tok = tokens[j]
        kw = kw_tok.text if kw_tok.kind == "IDENT" else ""

        # Handle `use ...;`
        if kw == "use":
            p_depth = 0
            b_depth = 0
            k = j + 1
            while k <= end_idx:
                tk = tokens[k]
                if tk.kind == "PUNCT":
                    if tk.text == "(":
                        p_depth += 1
                    elif tk.text == ")":
                        p_depth = max(0, p_depth - 1)
                    elif tk.text == "{":
                        b_depth += 1
                    elif tk.text == "}":
                        b_depth = max(0, b_depth - 1)
                    elif tk.text == ";" and p_depth == 0 and b_depth == 0:
                        break
                k += 1
            end_tok = tokens[min(k, end_idx)]
            if vis != "private":
                sig = _collapse_ws(source[tokens[item_start_idx].start : end_tok.end])
                items.append(
                    AstItem(
                        kind="pub_use",
                        name="use",
                        visibility=vis,
                        start_line=item_start_line,
                        end_line=end_tok.end_line,
                        signature=sig,
                        doc=_extract_doc_summary(pending_docs),
                        attrs=pending_attrs,
                        is_test=item_is_test,
                    )
                )
            i = k + 1
            continue

        # Handle `fn`
        if kw == "fn":
            fn_name = "<anon>"
            k = j + 1
            while k <= end_idx and tokens[k].kind in ("COMMENT_LINE", "COMMENT_BLOCK"):
                k += 1
            if k <= end_idx and tokens[k].kind == "IDENT":
                fn_name = tokens[k].text

            p_depth = 0
            brk_depth = 0
            ang_depth = 0
            seen_params = False
            body_open_idx = -1
            semi_idx = -1

            m = k
            while m <= end_idx:
                tk = tokens[m]
                if tk.kind in ("COMMENT_LINE", "COMMENT_BLOCK", "DOC_COMMENT"):
                    m += 1
                    continue
                if tk.kind == "PUNCT":
                    if tk.text == "(":
                        p_depth += 1
                        seen_params = True
                    elif tk.text == ")":
                        p_depth = max(0, p_depth - 1)
                    elif tk.text == "[":
                        brk_depth += 1
                    elif tk.text == "]":
                        brk_depth = max(0, brk_depth - 1)
                    elif tk.text == "<" and p_depth == 0 and brk_depth == 0:
                        # Avoid counting `->` as `<`
                        if m == 0 or tokens[m - 1].text != "-":
                            ang_depth += 1
                    elif tk.text == ">" and p_depth == 0 and brk_depth == 0:
                        if m == 0 or tokens[m - 1].text != "-":
                            ang_depth = max(0, ang_depth - 1)
                    elif tk.text == "{" and p_depth == 0 and brk_depth == 0 and seen_params:
                        body_open_idx = m
                        break
                    elif tk.text == ";" and p_depth == 0 and brk_depth == 0:
                        semi_idx = m
                        break
                m += 1

            if body_open_idx != -1:
                body_close_idx = _find_matching(tokens, body_open_idx, "{", "}")
                sig_raw = source[tokens[item_start_idx].start : tokens[body_open_idx].start]
                sig = _collapse_ws(sig_raw)
                end_line = tokens[body_close_idx].end_line
                items.append(
                    AstItem(
                        kind="fn",
                        name=fn_name,
                        visibility=vis,
                        start_line=item_start_line,
                        end_line=end_line,
                        signature=sig,
                        doc=_extract_doc_summary(pending_docs),
                        attrs=pending_attrs,
                        is_test=item_is_test,
                    )
                )
                i = body_close_idx + 1
                continue
            elif semi_idx != -1:
                sig_raw = source[tokens[item_start_idx].start : tokens[semi_idx].end]
                items.append(
                    AstItem(
                        kind="fn",
                        name=fn_name,
                        visibility=vis,
                        start_line=item_start_line,
                        end_line=tokens[semi_idx].end_line,
                        signature=_collapse_ws(sig_raw),
                        doc=_extract_doc_summary(pending_docs),
                        attrs=pending_attrs,
                        is_test=item_is_test,
                    )
                )
                i = semi_idx + 1
                continue

        # Handle `mod`, `struct`, `enum`, `trait`, `impl`
        if kw in ("mod", "struct", "enum", "trait", "impl"):
            name = "<impl>"
            if kw != "impl":
                k = j + 1
                while k <= end_idx and tokens[k].kind in ("COMMENT_LINE", "COMMENT_BLOCK"):
                    k += 1
                if k <= end_idx and tokens[k].kind == "IDENT":
                    name = tokens[k].text

            p_depth = 0
            brk_depth = 0
            open_brace_idx = -1
            semi_idx = -1
            m = j + 1
            while m <= end_idx:
                tk = tokens[m]
                if tk.kind in ("COMMENT_LINE", "COMMENT_BLOCK", "DOC_COMMENT"):
                    m += 1
                    continue
                if tk.kind == "PUNCT":
                    if tk.text == "(":
                        p_depth += 1
                    elif tk.text == ")":
                        p_depth = max(0, p_depth - 1)
                    elif tk.text == "[":
                        brk_depth += 1
                    elif tk.text == "]":
                        brk_depth = max(0, brk_depth - 1)
                    elif tk.text == "{" and p_depth == 0 and brk_depth == 0:
                        open_brace_idx = m
                        break
                    elif tk.text == ";" and p_depth == 0 and brk_depth == 0:
                        semi_idx = m
                        break
                m += 1

            if semi_idx != -1:
                sig = _collapse_ws(source[tokens[item_start_idx].start : tokens[semi_idx].end])
                items.append(
                    AstItem(
                        kind=kw,
                        name=name,
                        visibility=vis,
                        start_line=item_start_line,
                        end_line=tokens[semi_idx].end_line,
                        signature=sig,
                        doc=_extract_doc_summary(pending_docs),
                        attrs=pending_attrs,
                        is_test=item_is_test,
                    )
                )
                i = semi_idx + 1
                continue

            if open_brace_idx != -1:
                close_brace_idx = _find_matching(tokens, open_brace_idx, "{", "}")
                header_sig = _collapse_ws(source[tokens[item_start_idx].start : tokens[open_brace_idx].start])
                end_line = tokens[close_brace_idx].end_line

                if kw == "impl":
                    # Name is e.g. `SystemEngine` or `LogSource for FileSource`
                    impl_part = _collapse_ws(source[tokens[j + 1].start : tokens[open_brace_idx].start])
                    impl_part = re.sub(r"\s*where\b.*", "", impl_part).strip()
                    name = impl_part

                if kw in ("struct", "enum"):
                    body_lines = source[tokens[open_brace_idx].start : tokens[close_brace_idx].end].splitlines()
                    if len(body_lines) > 65:
                        kept = body_lines[:50] + [f"    // ... ({len(body_lines) - 52} lines omitted)", body_lines[-1]]
                        body_txt = "\n".join(kept)
                    else:
                        body_txt = "\n".join(body_lines)
                    items.append(
                        AstItem(
                            kind=kw,
                            name=name,
                            visibility=vis,
                            start_line=item_start_line,
                            end_line=end_line,
                            signature=header_sig,
                            doc=_extract_doc_summary(pending_docs),
                            attrs=pending_attrs,
                            is_test=item_is_test,
                            body_text=body_txt,
                        )
                    )
                else:
                    children = _scan_scope(
                        tokens,
                        source,
                        open_brace_idx + 1,
                        close_brace_idx - 1,
                        parent_is_test=item_is_test,
                    )
                    items.append(
                        AstItem(
                            kind=kw,
                            name=name,
                            visibility=vis,
                            start_line=item_start_line,
                            end_line=end_line,
                            signature=header_sig,
                            doc=_extract_doc_summary(pending_docs),
                            attrs=pending_attrs,
                            is_test=item_is_test,
                            children=children,
                        )
                    )
                i = close_brace_idx + 1
                continue

        # Handle `type`, `const`, `static`
        if kw in ("type", "const", "static"):
            item_name = "<const>"
            k = j + 1
            while k <= end_idx and tokens[k].kind in ("COMMENT_LINE", "COMMENT_BLOCK"):
                k += 1
            if kw == "static" and k <= end_idx and tokens[k].kind == "IDENT" and tokens[k].text == "mut":
                k += 1
            if k <= end_idx and tokens[k].kind == "IDENT":
                item_name = tokens[k].text

            p_depth = 0
            b_depth = 0
            brk_depth = 0
            m = j + 1
            while m <= end_idx:
                tk = tokens[m]
                if tk.kind == "PUNCT":
                    if tk.text == "(":
                        p_depth += 1
                    elif tk.text == ")":
                        p_depth = max(0, p_depth - 1)
                    elif tk.text == "{":
                        b_depth += 1
                    elif tk.text == "}":
                        b_depth = max(0, b_depth - 1)
                    elif tk.text == "[":
                        brk_depth += 1
                    elif tk.text == "]":
                        brk_depth = max(0, brk_depth - 1)
                    elif tk.text == ";" and p_depth == 0 and b_depth == 0 and brk_depth == 0:
                        break
                m += 1
            end_tok = tokens[min(m, end_idx)]
            sig = _collapse_ws(source[tokens[item_start_idx].start : end_tok.end])
            if len(sig) > 140:
                sig = sig[:137] + "..."
            items.append(
                AstItem(
                    kind=kw,
                    name=item_name,
                    visibility=vis,
                    start_line=item_start_line,
                    end_line=end_tok.end_line,
                    signature=sig,
                    doc=_extract_doc_summary(pending_docs),
                    attrs=pending_attrs,
                    is_test=item_is_test,
                )
            )
            i = m + 1
            continue

        # Fallback: macro_rules! or macro invocation (`lazy_static! { ... }`) or extern block
        p_depth = 0
        b_depth = 0
        brk_depth = 0
        m = j
        while m <= end_idx:
            tk = tokens[m]
            if tk.kind == "PUNCT":
                if tk.text == "(":
                    p_depth += 1
                elif tk.text == ")":
                    p_depth = max(0, p_depth - 1)
                elif tk.text == "[":
                    brk_depth += 1
                elif tk.text == "]":
                    brk_depth = max(0, brk_depth - 1)
                elif tk.text == "{":
                    b_depth += 1
                elif tk.text == "}":
                    b_depth -= 1
                    if b_depth <= 0 and p_depth == 0 and brk_depth == 0:
                        break
                elif tk.text == ";" and p_depth == 0 and b_depth == 0 and brk_depth == 0:
                    break
            m += 1
        if kw == "macro_rules" and m <= end_idx:
            macro_name = tokens[j + 2].text if (j + 2 <= end_idx and tokens[j + 2].kind == "IDENT") else "macro"
            items.append(
                AstItem(
                    kind="macro",
                    name=macro_name,
                    visibility=vis,
                    start_line=item_start_line,
                    end_line=tokens[min(m, end_idx)].end_line,
                    signature=f"macro_rules! {macro_name}",
                    doc=_extract_doc_summary(pending_docs),
                    attrs=pending_attrs,
                    is_test=item_is_test,
                )
            )
        i = m + 1

    return items


def parse_rust_skeleton(source: str) -> Tuple[List[AstItem], List[Token]]:
    tokens = tokenize_rust(source)
    if not tokens:
        return [], tokens
    items = _scan_scope(tokens, source, 0, len(tokens) - 1, parent_is_test=False)
    return items, tokens


def get_test_line_ranges(source: str) -> List[Tuple[int, int]]:
    """Return 1-based inclusive (start_line, end_line) for all #[cfg(test)] / #[test] AST items."""
    items, _ = parse_rust_skeleton(source)
    ranges: List[Tuple[int, int]] = []

    def walk(nodes: Sequence[AstItem]) -> None:
        for node in nodes:
            if node.is_test:
                ranges.append((node.start_line, node.end_line))
            elif node.children:
                walk(node.children)

    walk(items)
    return ranges


# ---------------------------------------------------------------------------
# Subcommand 1: tree
# ---------------------------------------------------------------------------

def _summarize_rs_file(path: Path) -> Tuple[int, str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    loc = len(text.splitlines())
    items, _ = parse_rust_skeleton(text)
    mods = [it.name for it in items if it.kind == "mod" and not it.is_test]
    types = [it.name for it in items if it.kind in ("struct", "enum", "trait") and not it.is_test]
    fns = [it.name for it in items if it.kind == "fn" and not it.is_test and it.visibility.startswith("pub")]

    parts: List[str] = []
    if mods:
        parts.append("mod: " + ", ".join(mods[:5]) + ("..." if len(mods) > 5 else ""))
    if types:
        parts.append("types: " + ", ".join(types[:5]) + ("..." if len(types) > 5 else ""))
    elif fns:
        parts.append("pub fn: " + ", ".join(fns[:4]) + ("..." if len(fns) > 4 else ""))
    summary = " | ".join(parts)
    return loc, summary


def cmd_tree(args: argparse.Namespace, repo_root: Path) -> int:
    crates = discover_crates(repo_root)
    if args.tier is not None:
        crates = [c for c in crates if c.tier == args.tier]
    if args.crate:
        q = args.crate.lower()
        crates = [c for c in crates if q in c.name.lower() or q in c.rel_dir.lower()]
    if args.target:
        target_norm = args.target.strip("/").lower()
        crates = [c for c in crates if target_norm in c.rel_dir.lower() or target_norm in c.name.lower()]

    if args.json:
        payload = []
        for c in sorted(crates, key=lambda x: (x.tier, x.name)):
            files = []
            for p in sorted(c.abs_dir.rglob("*.rs")):
                if any(part in IGNORED_DIRS for part in p.parts):
                    continue
                loc, summary = _summarize_rs_file(p)
                files.append({
                    "path": str(p.relative_to(repo_root)).replace("\\", "/"),
                    "loc": loc,
                    "summary": summary,
                })
            payload.append({
                "crate": c.name,
                "dir": c.rel_dir,
                "tier": c.tier,
                "internal_deps": c.internal_deps,
                "total_loc": sum(f["loc"] for f in files),
                "files": files,
            })
        print(json.dumps({"crates": payload}, indent=2))
        return 0

    total_loc = 0
    total_files = 0
    lines_out: List[str] = ["uwulog-rust Workspace Tree (Token-Optimized)"]

    grouped: Dict[int, List[CrateInfo]] = {}
    for c in crates:
        grouped.setdefault(c.tier, []).append(c)

    for tier in sorted(grouped.keys()):
        tier_label = grouped[tier][0].tier_label
        lines_out.append(f"\n=== [{tier_label}] ===")
        for c in sorted(grouped[tier], key=lambda x: x.name):
            deps_str = ", ".join(d.replace("uwu-", "") for d in c.internal_deps) or "none"
            rs_files = [
                p
                for p in sorted(c.abs_dir.rglob("*.rs"))
                if not any(part in IGNORED_DIRS for part in p.parts)
            ]
            crate_loc = 0
            file_lines: List[str] = []
            for p in rs_files:
                loc, summary = _summarize_rs_file(p)
                crate_loc += loc
                total_files += 1
                rel_p = str(p.relative_to(c.abs_dir)).replace("\\", "/")
                desc = f" [{summary}]" if summary else ""
                file_lines.append(f"  ├─ {rel_p} ({loc} LOC){desc}")
            total_loc += crate_loc
            lines_out.append(f"{c.rel_dir}/ ({c.name}, {crate_loc} LOC, deps: [{deps_str}])")
            lines_out.extend(file_lines)

    if not args.tier and not args.crate and not args.target:
        lines_out.append("\n=== [Workspace Auxiliary Directories] ===")
        for aux in [".agents", "assets/icons", "packaging", ".github/workflows"]:
            ap = repo_root / aux
            if ap.exists():
                cnt = sum(1 for _ in ap.rglob("*") if _.is_file())
                lines_out.append(f"  ├─ {aux}/ ({cnt} files)")

    lines_out.append(f"\nSummary: {len(crates)} crates, {total_files} .rs files, {total_loc} total LOC")
    print("\n".join(lines_out))
    return 0


# ---------------------------------------------------------------------------
# Subcommand 2: skeleton
# ---------------------------------------------------------------------------

def _render_items(
    items: Sequence[AstItem],
    indent: int,
    include_tests: bool,
    pub_only: bool,
    out: List[str],
) -> None:
    pad = "    " * indent
    for it in items:
        if it.is_test and not include_tests:
            if it.kind == "mod":
                if it.signature.endswith(";"):
                    out.append(f"{pad}#[cfg(test)] mod {it.name}; // [L{it.start_line}]")
                else:
                    test_count = sum(1 for ch in it.children if ch.kind == "fn")
                    out.append(
                        f"{pad}#[cfg(test)] mod {it.name} {{ /* {test_count} test fns omitted: L{it.start_line}-L{it.end_line}; pass --tests to expand */ }}"
                    )
            continue

        if pub_only and it.visibility == "private" and it.kind != "impl":
            continue

        for d in it.doc:
            out.append(f"{pad}/// {d}")
        important_attrs = [a for a in it.attrs if any(k in a for k in ("cfg(", "test", "derive(", "command("))]
        for a in important_attrs[:2]:
            out.append(f"{pad}{a}")

        if it.kind in ("struct", "enum") and it.body_text:
            body_lines = it.body_text.splitlines()
            out.append(f"{pad}{it.signature} {body_lines[0]} // [L{it.start_line}-L{it.end_line}]")
            for bl in body_lines[1:]:
                out.append(f"{pad}{bl}")
        elif it.kind in ("impl", "trait", "mod"):
            if it.children or it.signature.endswith("}"):
                out.append(f"{pad}{it.signature} {{ // [L{it.start_line}-L{it.end_line}]")
                _render_items(it.children, indent + 1, include_tests, pub_only, out)
                out.append(f"{pad}}}")
            else:
                out.append(f"{pad}{it.signature} // [L{it.start_line}]")
        elif it.kind == "fn":
            if it.signature.endswith(";"):
                out.append(f"{pad}{it.signature} // [L{it.start_line}-L{it.end_line}]")
            else:
                out.append(f"{pad}{it.signature} {{ /* L{it.start_line}-L{it.end_line} */ }}")
        else:
            out.append(f"{pad}{it.signature} // [L{it.start_line}-L{it.end_line}]")


def _resolve_target_rs_files(repo_root: Path, target: str) -> List[Path]:
    p = Path(target)
    if not p.is_absolute():
        p = repo_root / target
    if p.is_file():
        return [p]
    if p.is_dir():
        return sorted(f for f in p.rglob("*.rs") if not any(d in IGNORED_DIRS for d in f.parts))

    # Try matching crate name
    crates = discover_crates(repo_root)
    q = target.lower().replace("_", "-")
    for c in crates:
        if c.name.lower() == q or Path(c.rel_dir).name.lower().replace("_", "-") == q:
            return sorted(f for f in c.abs_dir.rglob("*.rs") if not any(d in IGNORED_DIRS for d in f.parts))
    return []


def cmd_skeleton(args: argparse.Namespace, repo_root: Path) -> int:
    files = _resolve_target_rs_files(repo_root, args.target)
    if not files:
        print(f"Error: Target '{args.target}' not found as a .rs file, directory, or crate.", file=sys.stderr)
        return 1

    crates = discover_crates(repo_root)
    json_payload = []
    all_output: List[str] = []
    orig_total_lines = 0
    shown_total_lines = 0

    for fpath in files:
        source = fpath.read_text(encoding="utf-8", errors="replace")
        loc = len(source.splitlines())
        orig_total_lines += loc
        items, _ = parse_rust_skeleton(source)
        rel = str(fpath.relative_to(repo_root)).replace("\\", "/")

        tier_tag = ""
        for c in crates:
            if rel.startswith(c.rel_dir + "/"):
                tier_tag = f" [{c.tier_label} | {c.name}]"
                break

        if args.json:
            def to_dict(it: AstItem) -> dict:
                return {
                    "kind": it.kind,
                    "name": it.name,
                    "visibility": it.visibility,
                    "start_line": it.start_line,
                    "end_line": it.end_line,
                    "signature": it.signature,
                    "is_test": it.is_test,
                    "children": [to_dict(ch) for ch in it.children],
                }

            json_payload.append({
                "file": rel,
                "total_lines": loc,
                "items": [to_dict(it) for it in items if args.tests or not it.is_test],
            })
        else:
            rendered: List[str] = [f"// === {rel} ({loc} lines){tier_tag} ==="]
            _render_items(items, 0, args.tests, args.pub_only, rendered)
            shown_total_lines += len(rendered)
            all_output.extend(rendered)
            all_output.append("")

    if args.json:
        print(json.dumps({"files": json_payload}, indent=2))
        return 0

    reduction = 100.0 * (1.0 - (shown_total_lines / max(1, orig_total_lines)))
    all_output.append(
        f"// Token-Optimizer Summary: {shown_total_lines} skeleton lines vs {orig_total_lines} source lines ({reduction:.1f}% reduction)"
    )
    print("\n".join(all_output))
    return 0


# ---------------------------------------------------------------------------
# Subcommand 3: symbol
# ---------------------------------------------------------------------------

def _enclosing_fn(items: Sequence[AstItem], line: int, prefix: str = "") -> Optional[str]:
    for it in items:
        if it.start_line <= line <= it.end_line:
            if it.kind == "fn":
                return f"{prefix}{it.name}"
            if it.kind in ("impl", "mod", "trait") and it.children:
                sub_prefix = f"{it.name}::" if it.kind in ("impl", "mod") else ""
                inner = _enclosing_fn(it.children, line, prefix + sub_prefix)
                if inner:
                    return inner
                return f"{prefix}{it.kind} {it.name}"
    return None


def cmd_symbol(args: argparse.Namespace, repo_root: Path) -> int:
    crates = discover_crates(repo_root)
    if args.crate:
        q = args.crate.lower()
        crates = [c for c in crates if q in c.name.lower() or q in c.rel_dir.lower()]

    query = args.name
    defs: List[dict] = []
    refs: List[dict] = []

    def collect_defs(items: Sequence[AstItem], rel: str, crate_name: str, tier: int, scope: str = "") -> None:
        for it in items:
            matched = (it.name == query) if not args.fuzzy else (query.lower() in it.name.lower())
            if matched and it.kind != "pub_use":
                defs.append({
                    "crate": crate_name,
                    "tier": tier,
                    "file": rel,
                    "kind": it.kind,
                    "name": it.name,
                    "scope": scope,
                    "start_line": it.start_line,
                    "end_line": it.end_line,
                    "signature": it.signature,
                })
            if it.children:
                next_scope = f"{scope}::{it.name}" if scope else it.name
                collect_defs(it.children, rel, crate_name, tier, next_scope)

    for c in crates:
        for fpath in sorted(c.abs_dir.rglob("*.rs")):
            if any(part in IGNORED_DIRS for part in fpath.parts):
                continue
            rel = str(fpath.relative_to(repo_root)).replace("\\", "/")
            source = fpath.read_text(encoding="utf-8", errors="replace")
            items, tokens = parse_rust_skeleton(source)
            collect_defs(items, rel, c.name, c.tier)

            if not args.defs_only:
                lines = source.splitlines()
                seen_lines = set()
                for tok in tokens:
                    if tok.kind != "IDENT":
                        continue
                    is_match = (tok.text == query) if not args.fuzzy else (query.lower() in tok.text.lower())
                    if not is_match or tok.line in seen_lines:
                        continue
                    seen_lines.add(tok.line)
                    # Skip if this exact line is already the definition line
                    if any(d["file"] == rel and d["start_line"] == tok.line for d in defs):
                        continue
                    enc = _enclosing_fn(items, tok.line)
                    snippet = lines[tok.line - 1].strip() if 0 < tok.line <= len(lines) else ""
                    refs.append({
                        "crate": c.name,
                        "tier": c.tier,
                        "file": rel,
                        "line": tok.line,
                        "enclosing": enc or "top-level",
                        "snippet": snippet[:140],
                    })

    if args.json:
        print(json.dumps({"symbol": query, "definitions": defs, "references": refs[: args.limit], "total_references": len(refs)}, indent=2))
        return 0

    out: List[str] = [f"=== Symbol Lookup: '{query}' ==="]
    out.append(f"\n[Definitions ({len(defs)})]")
    if not defs:
        out.append("  (none found)")
    for d in defs:
        scope_str = f" (in {d['scope']})" if d["scope"] else ""
        out.append(f"  • [Tier {d['tier']}] {d['crate']} :: {d['file']}:L{d['start_line']}-L{d['end_line']}{scope_str}")
        out.append(f"    {d['signature']}")

    if not args.defs_only:
        out.append(f"\n[References / Usages (showing {min(len(refs), args.limit)} of {len(refs)})]")
        for r in refs[: args.limit]:
            out.append(f"  • [Tier {r['tier']}] {r['file']}:L{r['line']} ({r['enclosing']}): {r['snippet']}")
        if len(refs) > args.limit:
            out.append(f"  ... ({len(refs) - args.limit} more references; pass --limit N to expand)")

    print("\n".join(out))
    return 0


# ---------------------------------------------------------------------------
# Subcommand 4: hotspots
# ---------------------------------------------------------------------------

def cmd_hotspots(args: argparse.Namespace, repo_root: Path) -> int:
    crates = discover_crates(repo_root)
    if args.target:
        q = args.target.lower().replace("_", "-")
        crates = [
            c
            for c in crates
            if q in c.name.lower() or q in c.rel_dir.lower() or args.target in c.rel_dir
        ]

    large_fns: List[dict] = []
    large_files: List[dict] = []
    concurrency_sites: List[dict] = []

    sync_keywords = ("RwLock", "Mutex", "AtomicU64", "AtomicBool", "par_iter", "tokio::spawn", "mpsc::channel", "unsafe")

    for c in crates:
        for fpath in sorted(c.abs_dir.rglob("*.rs")):
            if any(part in IGNORED_DIRS for part in fpath.parts):
                continue
            rel = str(fpath.relative_to(repo_root)).replace("\\", "/")
            if "/tests/" in rel and not args.tests:
                continue
            source = fpath.read_text(encoding="utf-8", errors="replace")
            lines = source.splitlines()
            loc = len(lines)
            items, tokens = parse_rust_skeleton(source)

            if loc >= 250:
                large_files.append({"crate": c.name, "tier": c.tier, "file": rel, "loc": loc})

            def walk_fns(nodes: Sequence[AstItem], scope: str = "") -> None:
                for it in nodes:
                    if it.is_test and not args.tests:
                        continue
                    if it.kind == "fn":
                        span = it.end_line - it.start_line + 1
                        large_fns.append({
                            "crate": c.name,
                            "tier": c.tier,
                            "file": rel,
                            "fn": f"{scope}::{it.name}" if scope else it.name,
                            "lines": span,
                            "start_line": it.start_line,
                            "end_line": it.end_line,
                        })
                    elif it.children:
                        next_scope = f"{scope}::{it.name}" if scope else it.name
                        walk_fns(it.children, next_scope)

            walk_fns(items)

            test_ranges = get_test_line_ranges(source) if not args.tests else []
            for idx, line_txt in enumerate(lines, start=1):
                if any(s <= idx <= e for s, e in test_ranges):
                    continue
                stripped = line_txt.strip()
                if stripped.startswith("//"):
                    continue
                hits = [kw for kw in sync_keywords if kw in stripped]
                if hits:
                    concurrency_sites.append({
                        "crate": c.name,
                        "file": rel,
                        "line": idx,
                        "markers": hits,
                        "snippet": stripped[:120],
                    })

    large_fns.sort(key=lambda x: x["lines"], reverse=True)
    large_files.sort(key=lambda x: x["loc"], reverse=True)

    top_n = args.top
    if args.json:
        print(json.dumps({
            "largest_files": large_files[:top_n],
            "largest_functions": large_fns[:top_n],
            "concurrency_and_unsafe": concurrency_sites[: top_n * 2],
        }, indent=2))
        return 0

    out: List[str] = ["=== Codebase Hotspots & Concurrency Map ==="]
    out.append(f"\n[Largest Files (>= 250 LOC, top {top_n})]")
    for f in large_files[:top_n]:
        out.append(f"  • {f['loc']:4d} LOC | [Tier {f['tier']}] {f['file']}")

    out.append(f"\n[Longest Functions (top {top_n})]")
    for fn in large_fns[:top_n]:
        out.append(
            f"  • {fn['lines']:4d} lines | [Tier {fn['tier']}] {fn['file']}:L{fn['start_line']}-L{fn['end_line']} — {fn['fn']}"
        )

    out.append(f"\n[Concurrency, Parallelism & Unsafe Sites (showing {min(len(concurrency_sites), top_n * 2)} of {len(concurrency_sites)})]")
    for site in concurrency_sites[: top_n * 2]:
        markers = ",".join(site["markers"])
        out.append(f"  • {site['file']}:L{site['line']} [{markers}] {site['snippet']}")

    print("\n".join(out))
    return 0


# ---------------------------------------------------------------------------
# Subcommand 5 & 6: fields (StandardField SSOT) & keymap (KeyAction / KeyContext)
# ---------------------------------------------------------------------------

def _extract_fn_source(source: str, fn_name: str) -> str:
    items, _ = parse_rust_skeleton(source)
    lines = source.splitlines()

    def find_fn(nodes: Sequence[AstItem]) -> Optional[AstItem]:
        for it in nodes:
            if it.kind == "fn" and it.name == fn_name and not it.is_test:
                return it
            if it.children:
                found = find_fn(it.children)
                if found:
                    return found
        return None

    target = find_fn(items)
    if not target:
        return ""
    return "\n".join(lines[target.start_line - 1 : target.end_line])


def cmd_fields(args: argparse.Namespace, repo_root: Path) -> int:
    schema_file = repo_root / "crates" / "core_schema" / "src" / "lib.rs"
    builder_file = repo_root / "crates" / "core_engine" / "src" / "arrow_storage" / "builder.rs"

    schema_src = schema_file.read_text(encoding="utf-8", errors="replace")
    builder_src = builder_file.read_text(encoding="utf-8", errors="replace")

    canonical_map: Dict[str, str] = {}
    canonical_src = _extract_fn_source(schema_src, "canonical_name")
    for var, cname in re.findall(r'Self::(\w+)\s*=>\s*"([^"]+)"', canonical_src):
        canonical_map[var] = cname

    alias_map: Dict[str, List[str]] = {}
    alias_src = _extract_fn_source(schema_src, "from_alias")
    for pat, var in re.findall(
        r'((?:"[^"]+"\s*\|\s*)*"[^"]+")\s*=>\s*\{?\s*Some\(Self::(\w+)\)\s*\}?',
        alias_src,
        re.S,
    ):
        aliases = re.findall(r'"([^"]+)"', pat)
        alias_map[var] = aliases

    type_map: Dict[str, str] = {}
    ft_src = _extract_fn_source(schema_src, "field_type")
    for lhs, rhs in re.findall(r"((?:Self::\w+\s*\|\s*)*Self::\w+)\s*=>\s*FieldType::(\w+)", ft_src):
        for var in re.findall(r"Self::(\w+)", lhs):
            type_map[var] = rhs

    arrow_cols: List[Dict[str, object]] = []
    for col_name, dtype, nullable in re.findall(
        r'Field::new\("(__[^"]+)",\s*DataType::(\w+),\s*(true|false)\)',
        builder_src,
    ):
        arrow_cols.append({"column": col_name, "arrow_type": dtype, "nullable": nullable == "true"})

    fields_out = []
    for var in ("Timestamp", "Level", "Message", "Id"):
        fields_out.append({
            "variant": f"StandardField::{var}",
            "canonical_name": canonical_map.get(var, var.lower()),
            "field_type": type_map.get(var, "Unknown"),
            "aliases": alias_map.get(var, []),
        })

    query = (getattr(args, "filter", None) or "").strip().lower()
    if query:
        fields_out = [
            f
            for f in fields_out
            if query in f["variant"].lower()
            or query in f["canonical_name"].lower()
            or any(query in a.lower() for a in f["aliases"])
        ]

    payload = {
        "ssot_source": "crates/core_schema/src/lib.rs",
        "standard_fields": fields_out,
        "arrow_core_columns": arrow_cols,
    }
    if getattr(args, "json", False):
        print(json.dumps(payload, indent=2, ensure_ascii=False))
        return 0

    out = ["=== SSOT StandardField & Arrow System Columns ==="]
    for f in fields_out:
        out.append(
            f"  • {f['variant']} -> canonical='{f['canonical_name']}' ({f['field_type']}) | aliases: {', '.join(f['aliases'])}"
        )
    out.append("\n[Arrow Internal System Columns (builder.rs)]")
    for c in arrow_cols:
        null_tag = "nullable" if c["nullable"] else "non-null"
        out.append(f"  • {c['column']} : DataType::{c['arrow_type']} ({null_tag})")
    print("\n".join(out))
    return 0


def cmd_keymap(args: argparse.Namespace, repo_root: Path) -> int:
    action_file = repo_root / "crates" / "core_keymap" / "src" / "action.rs"
    context_file = repo_root / "crates" / "core_keymap" / "src" / "context.rs"
    manager_file = repo_root / "crates" / "core_keymap" / "src" / "manager.rs"

    action_src = action_file.read_text(encoding="utf-8", errors="replace")
    context_src = context_file.read_text(encoding="utf-8", errors="replace")
    manager_src = manager_file.read_text(encoding="utf-8", errors="replace")

    actions: List[Dict[str, object]] = []
    m_act = re.search(r"define_actions!\s*\{(.*?)\n\}", action_src, re.S)
    if m_act:
        for var, canonical, alias_blk in re.findall(
            r'(\w+)\s*=>\s*\("([^"]+)",\s*\[(.*?)\]\)',
            m_act.group(1),
            re.S,
        ):
            aliases = re.findall(r'"([^"]+)"', alias_blk)
            actions.append({"variant": var, "canonical": canonical, "aliases": aliases})

    contexts: List[str] = []
    m_ctx = re.search(r"ALL_BUILTIN_CONTEXTS:\s*&\[KeyContext\]\s*=\s*&\[(.*?)\];", context_src, re.S)
    if m_ctx:
        contexts = re.findall(r"KeyContext::(\w+)", m_ctx.group(1))

    defaults: List[Dict[str, str]] = []
    reg_src = _extract_fn_source(manager_src, "register_defaults")
    for ks_raw, act, ctx in re.findall(
        r"self\.bind_keystroke_internal\(\s*(Keystroke::[^;]+?),\s*KeyAction::(\w+),\s*KeyContext::(\w+),?\s*\);",
        reg_src,
        re.S,
    ):
        ks_clean = re.sub(r"\s+", " ", ks_raw).strip()
        defaults.append({"keystroke": ks_clean, "action": act, "context": ctx})

    q = (getattr(args, "query", None) or "").strip().lower()
    ctx_filter = (getattr(args, "context", None) or "").strip().lower()

    if q:
        actions = [
            a
            for a in actions
            if q in str(a["variant"]).lower()
            or q in str(a["canonical"]).lower()
            or any(q in al.lower() for al in a["aliases"])  # type: ignore[union-attr]
        ]
        defaults = [
            d
            for d in defaults
            if q in d["action"].lower() or q in d["keystroke"].lower() or q in d["context"].lower()
        ]
    if ctx_filter:
        defaults = [d for d in defaults if ctx_filter in d["context"].lower()]

    payload = {
        "builtin_contexts": contexts,
        "actions_count": len(actions),
        "actions": actions,
        "default_bindings_count": len(defaults),
        "default_bindings": defaults,
    }
    if getattr(args, "json", False):
        print(json.dumps(payload, indent=2, ensure_ascii=False))
        return 0

    out = [f"=== Zed-Style 4-Layer Keymap Lookup (contexts: {', '.join(contexts)}) ==="]
    out.append(f"\n[KeyAction Variants ({len(actions)})]")
    for a in actions:
        alias_str = f" (aliases: {', '.join(a['aliases'])})" if a["aliases"] else ""  # type: ignore[arg-type]
        out.append(f"  • KeyAction::{a['variant']} => \"{a['canonical']}\"{alias_str}")
    out.append(f"\n[Default Bindings ({len(defaults)})]")
    for d in defaults:
        out.append(f"  • [{d['context']}] {d['keystroke']} -> KeyAction::{d['action']}")
    print("\n".join(out))
    return 0


def collect_changed_files(repo_root: Path, base_ref: Optional[str] = None) -> List[str]:
    import subprocess

    changed: set[str] = set()
    try:
        if base_ref:
            res = subprocess.run(
                ["git", "diff", "--name-only", base_ref],
                cwd=repo_root,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )
            if res.returncode == 0:
                for line in res.stdout.splitlines():
                    p = line.strip().replace("\\", "/")
                    if p:
                        changed.add(p)
        else:
            st = subprocess.run(
                ["git", "status", "--porcelain"],
                cwd=repo_root,
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )
            if st.returncode == 0:
                for raw_line in st.stdout.splitlines():
                    if len(raw_line) < 4:
                        continue
                    path_part = raw_line[3:].strip()
                    if " -> " in path_part:
                        path_part = path_part.split(" -> ", 1)[1].strip()
                    path_part = path_part.strip('"').replace("\\", "/")
                    if path_part:
                        changed.add(path_part)
    except Exception:
        pass
    return sorted(changed)


def compute_affected_crates(
    repo_root: Path,
    changed_files: Optional[Sequence[str]] = None,
    include_downstream: bool = False,
) -> Dict[str, object]:
    crates = discover_crates(repo_root)
    dir_to_crate = {c.rel_dir.rstrip("/"): c for c in crates}
    name_to_crate = {c.name: c for c in crates}

    files = list(changed_files) if changed_files is not None else collect_changed_files(repo_root)
    direct: set[str] = set()
    test_all = False

    for raw in files:
        norm = raw.replace("\\", "/").lstrip("./")
        if norm in ("Cargo.toml", "Cargo.lock"):
            test_all = True
            continue
        if norm.startswith("assets/icons/"):
            if "uwu-icons" in name_to_crate:
                direct.add("uwu-icons")
            continue
        for rel_dir, cinfo in dir_to_crate.items():
            if norm == rel_dir or norm.startswith(rel_dir + "/"):
                direct.add(cinfo.name)
                break

    downstream: set[str] = set()
    if include_downstream and not test_all and direct:
        rev_deps: Dict[str, List[str]] = {}
        for c in crates:
            for dep in c.internal_deps:
                rev_deps.setdefault(dep, []).append(c.name)

        queue = list(direct)
        seen = set(direct)
        while queue:
            cur = queue.pop(0)
            for nxt in rev_deps.get(cur, []):
                if nxt not in seen:
                    seen.add(nxt)
                    downstream.add(nxt)
                    queue.append(nxt)

    def _sort_key(cname: str) -> Tuple[int, str]:
        ci = name_to_crate.get(cname)
        return (ci.tier if ci else 99, cname)

    direct_sorted = sorted(direct, key=_sort_key)
    downstream_sorted = sorted(downstream, key=_sort_key)
    all_affected = (
        sorted(name_to_crate.keys(), key=_sort_key)
        if test_all
        else sorted(direct | downstream, key=_sort_key)
    )

    return {
        "changed_files": files,
        "test_all": test_all,
        "include_downstream": include_downstream,
        "direct_crates": direct_sorted,
        "downstream_crates": downstream_sorted,
        "affected_crates": all_affected,
    }


def cmd_affected(args: argparse.Namespace, repo_root: Path) -> int:
    changed = collect_changed_files(repo_root, base_ref=getattr(args, "base", None))
    info = compute_affected_crates(
        repo_root,
        changed_files=changed,
        include_downstream=bool(getattr(args, "downstream", False)),
    )
    if getattr(args, "json", False):
        print(json.dumps(info, indent=2, ensure_ascii=False))
        return 0

    if info["test_all"]:
        print("Affected crates: ALL (root Cargo.toml/Cargo.lock changed)")
        return 0

    affected = info["affected_crates"]
    if not affected:
        print("Affected crates: (none)")
        return 0

    direct = info["direct_crates"]
    downstream = info["downstream_crates"]
    out = [f"Affected crates ({len(affected)}): {', '.join(affected)}"]  # type: ignore[arg-type]
    out.append(f"  Direct: {', '.join(direct) if direct else '(none)'}")  # type: ignore[arg-type]
    if info["include_downstream"]:
        out.append(f"  Downstream: {', '.join(downstream) if downstream else '(none)'}")  # type: ignore[arg-type]
    print("\n".join(out))
    return 0


# ---------------------------------------------------------------------------
# CLI Entrypoint
# ---------------------------------------------------------------------------

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="Token-optimized Rust AST & directory indexer for uwulog-rust")
    sub = p.add_subparsers(dest="command", required=True)

    p_tree = sub.add_parser("tree", help="Workspace & crate structure tree")
    p_tree.add_argument("target", nargs="?", default=None, help="Optional crate or directory filter")
    p_tree.add_argument("--tier", type=int, choices=[0, 1, 2, 3, 4], default=None)
    p_tree.add_argument("-p", "--crate", default=None)
    p_tree.add_argument("--json", action="store_true")

    p_skel = sub.add_parser("skeleton", help="Extract AST skeleton without fn bodies")
    p_skel.add_argument("target", help="File path, directory, or crate name")
    p_skel.add_argument("--tests", action="store_true", help="Include #[cfg(test)] items")
    p_skel.add_argument("--pub-only", action="store_true", help="Only include pub items and impls")
    p_skel.add_argument("--json", action="store_true")

    p_sym = sub.add_parser("symbol", help="Locate symbol definitions & cross-crate references")
    p_sym.add_argument("name", help="Exact symbol identifier (or substring with --fuzzy)")
    p_sym.add_argument("-p", "--crate", default=None)
    p_sym.add_argument("--defs-only", action="store_true")
    p_sym.add_argument("--fuzzy", action="store_true")
    p_sym.add_argument("--limit", type=int, default=40)
    p_sym.add_argument("--json", action="store_true")

    p_hot = sub.add_parser("hotspots", help="Find largest functions, files, locks, par_iter, unsafe")
    p_hot.add_argument("target", nargs="?", default=None, help="Optional crate or directory filter")
    p_hot.add_argument("--top", type=int, default=15)
    p_hot.add_argument("--tests", action="store_true")
    p_hot.add_argument("--json", action="store_true")

    p_fld = sub.add_parser("fields", help="Inspect StandardField SSOT & Arrow system columns")
    p_fld.add_argument("filter", nargs="?", default=None, help="Optional substring filter")
    p_fld.add_argument("--json", action="store_true")

    p_km = sub.add_parser("keymap", help="Inspect KeyAction, KeyContext & default bindings")
    p_km.add_argument("query", nargs="?", default=None, help="Optional action/keystroke filter")
    p_km.add_argument("-c", "--context", default=None, help="Optional KeyContext filter")
    p_km.add_argument("--json", action="store_true")

    p_aff = sub.add_parser("affected", help="Detect workspace crates affected by git changes")
    p_aff.add_argument("--downstream", action="store_true", help="Include downstream crates via reverse 5-Tier DAG")
    p_aff.add_argument("--base", default=None, help="Optional git base ref (defaults to working tree status)")
    p_aff.add_argument("--json", action="store_true")

    return p


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    repo_root = find_repo_root()

    if args.command == "tree":
        return cmd_tree(args, repo_root)
    if args.command == "skeleton":
        return cmd_skeleton(args, repo_root)
    if args.command == "symbol":
        return cmd_symbol(args, repo_root)
    if args.command == "hotspots":
        return cmd_hotspots(args, repo_root)
    if args.command == "fields":
        return cmd_fields(args, repo_root)
    if args.command == "keymap":
        return cmd_keymap(args, repo_root)
    if args.command == "affected":
        return cmd_affected(args, repo_root)
    return 1


if __name__ == "__main__":
    sys.exit(main())
