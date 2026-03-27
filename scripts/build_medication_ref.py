#!/usr/bin/env python3
"""
Build the medication reference SQLite from Swissmedic AIPS XML data.

The generated file is published as a GitHub Release asset alongside a detached
minisign signature.  The app downloads both, verifies the signature with a
hardcoded public key, and uses the SQLite for local-only autocomplete searches.

Usage (run in CI after downloading AIPS XML):
    python scripts/build_medication_ref.py \
        --aips aips_de.xml \
        --out  medication_ref_de.sqlite \
        --version "2025-03"

Dependencies: lxml (pip install lxml)
"""

import argparse
import json
import re
import sqlite3
import sys
from pathlib import Path

try:
    from lxml import etree
except ImportError:
    print("ERROR: lxml is required.  Run: pip install lxml", file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# XML helpers
# ---------------------------------------------------------------------------

def text_or_none(element, xpath: str) -> str | None:
    """Return stripped text of the first matching element, or None."""
    nodes = element.xpath(xpath)
    if not nodes:
        return None
    # Flatten all text content (handles mixed-content nodes with nested elements)
    raw = etree.tostring(nodes[0], method="text", encoding="unicode")
    text = re.sub(r"\s+", " ", raw).strip()
    return text or None


def trade_names_from_element(med_element) -> list[str]:
    """Extract all registered trade names for this substance entry."""
    names: list[str] = []
    for name_el in med_element.xpath(".//name"):
        t = (name_el.text or "").strip()
        if t:
            names.append(t)
    return list(dict.fromkeys(names))  # deduplicate, preserve order


# ---------------------------------------------------------------------------
# AIPS XML parsing
# ---------------------------------------------------------------------------

def parse_aips_xml(xml_path: Path) -> list[dict]:
    """
    Parse the AIPS mediXML export and return a list of substance dicts.

    AIPS mediXML structure (simplified):
        <medicalInformations>
          <medicalInformation type="fi" lang="de">
            <title>Trade name</title>
            <authHolder>Zulassungsinhaber AG</authHolder>
            <atcCode>N06AB06</atcCode>
            <substances>
              <substance>
                <name>Sertralin</name>
              </substance>
            </substances>
            <paragraphs>
              <paragraph type="section5">...</paragraph>  <!-- Indications -->
              <paragraph type="section4.3">...</paragraph>  <!-- Contraindications -->
              <paragraph type="section4.8">...</paragraph>  <!-- Side effects -->
            </paragraphs>
          </medicalInformation>
        </medicalInformations>
    """
    tree = etree.parse(str(xml_path))
    root = tree.getroot()

    substances: dict[str, dict] = {}  # keyed by lowercase substance name (deduplicate)

    for med in root.xpath("//medicalInformation[@type='fi' and @lang='de']"):
        atc = text_or_none(med, "atcCode")
        trade_name = text_or_none(med, "title")

        # Each FI entry lists one or more active substances
        for subst_el in med.xpath(".//substances/substance"):
            name = text_or_none(subst_el, "name")
            if not name:
                continue

            key = name.lower()
            if key not in substances:
                substances[key] = {
                    "id": key,
                    "name_de": name,
                    "atc_code": atc,
                    "trade_names": [],
                    "indication": text_or_none(med, ".//paragraph[@type='section5']"),
                    "side_effects": text_or_none(med, ".//paragraph[@type='section4.8']"),
                    "contraindications": text_or_none(med, ".//paragraph[@type='section4.3']"),
                }

            entry = substances[key]
            if trade_name and trade_name not in entry["trade_names"]:
                entry["trade_names"].append(trade_name)
            # First non-null ATC code wins
            if atc and not entry["atc_code"]:
                entry["atc_code"] = atc

    return list(substances.values())


# ---------------------------------------------------------------------------
# SQLite writer
# ---------------------------------------------------------------------------

def write_sqlite(records: list[dict], out_path: Path, version: str) -> None:
    if out_path.exists():
        out_path.unlink()

    conn = sqlite3.connect(str(out_path))
    conn.execute("PRAGMA journal_mode = DELETE;")  # single-writer, read-only in prod
    conn.execute("PRAGMA synchronous = FULL;")

    conn.executescript("""
        CREATE TABLE substances (
            id                TEXT PRIMARY KEY NOT NULL,
            name_de           TEXT NOT NULL,
            atc_code          TEXT,
            trade_names       TEXT,
            indication        TEXT,
            side_effects      TEXT,
            contraindications TEXT,
            source_version    TEXT
        );

        CREATE VIRTUAL TABLE substances_fts USING fts5(
            name_de,
            trade_names,
            content='substances',
            content_rowid='rowid',
            tokenize='unicode61 remove_diacritics 1'
        );
    """)

    for rec in records:
        conn.execute(
            """INSERT INTO substances
               (id, name_de, atc_code, trade_names,
                indication, side_effects, contraindications, source_version)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)""",
            (
                rec["id"],
                rec["name_de"],
                rec.get("atc_code"),
                json.dumps(rec.get("trade_names", []), ensure_ascii=False),
                _truncate(rec.get("indication"), 2000),
                _truncate(rec.get("side_effects"), 3000),
                _truncate(rec.get("contraindications"), 2000),
                version,
            ),
        )

    # Populate FTS index
    conn.execute(
        """INSERT INTO substances_fts(rowid, name_de, trade_names)
           SELECT rowid, name_de, COALESCE(trade_names, '') FROM substances"""
    )

    conn.commit()
    conn.execute("PRAGMA optimize;")
    conn.execute("VACUUM;")
    conn.close()

    size_kb = out_path.stat().st_size // 1024
    print(f"Wrote {len(records)} substances to '{out_path}' ({size_kb} KB, version={version})")


def _truncate(text: str | None, max_len: int) -> str | None:
    if text is None:
        return None
    if len(text) <= max_len:
        return text
    return text[:max_len].rsplit(" ", 1)[0] + "…"


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--aips", required=True, help="Path to AIPS mediXML file (e.g. aips_de.xml)")
    ap.add_argument("--out", default="medication_ref_de.sqlite", help="Output SQLite path")
    ap.add_argument("--version", required=True, help="Source version string (e.g. 2025-03)")
    args = ap.parse_args()

    aips_path = Path(args.aips)
    if not aips_path.exists():
        print(f"ERROR: AIPS XML not found: {aips_path}", file=sys.stderr)
        sys.exit(1)

    print(f"Parsing {aips_path} …")
    records = parse_aips_xml(aips_path)
    print(f"Found {len(records)} unique substances.")

    write_sqlite(records, Path(args.out), args.version)


if __name__ == "__main__":
    main()
