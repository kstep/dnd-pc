#!/usr/bin/env python3
"""
Split locale-specific data files into:
  - public/data/  (mechanics only, no text)
  - public/{locale}/  (flat locale maps)

Usage: python3 scripts/split_data.py
"""

import json
import os
import sys
from collections import OrderedDict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "public"
LOCALES = ["en", "ru"]
TEXT_FIELDS = {"label", "description", "short"}


def extract_text(obj, text_fields=TEXT_FIELDS):
    """Extract text fields from an object, returning (locale_text, cleaned_obj)."""
    text = {}
    for field in text_fields:
        if field in obj:
            val = obj[field]
            if field == "label":
                # label is optional - only include if present and non-null
                if val is not None:
                    text[field] = val
            elif field == "description":
                # description is always a string; include if non-empty
                if val:
                    text[field] = val
            elif field == "short":
                if val is not None:
                    text[field] = val
    return text


def strip_text(obj, text_fields=TEXT_FIELDS):
    """Return a copy of obj without text fields."""
    result = OrderedDict()
    for k, v in obj.items():
        if k == "description":
            result[k] = ""  # keep field with empty string (required field)
        elif k in text_fields:
            continue  # skip optional text fields
        else:
            result[k] = v
    return result


def process_feature(feature, prefix, locale_map):
    """Process a feature definition, extracting text into locale_map."""
    name = feature.get("name", "")
    if not name:
        return strip_text(feature)

    key = f"{prefix}.{name}" if prefix else f"feature.{name}"

    # Extract feature-level text
    text = extract_text(feature)
    if text:
        locale_map[key] = text

    cleaned = strip_text(feature)

    # Process fields
    if "fields" in feature:
        cleaned_fields = []
        for field in feature["fields"]:
            fname = field.get("name", "")
            if fname:
                field_key = f"{key}.field.{fname}"
                # For Points fields, also extract "short"
                field_text = extract_text(field)
                if "kind" in field and field["kind"] == "Points" and "short" in field:
                    field_text["short"] = field["short"]
                if field_text:
                    locale_map[field_key] = field_text

                # Process choice options
                if "options" in field and isinstance(field["options"], list):
                    cleaned_options = []
                    for opt in field["options"]:
                        oname = opt.get("name", "")
                        if oname:
                            opt_key = f"{field_key}.option.{oname}"
                            opt_text = extract_text(opt)
                            if opt_text:
                                locale_map[opt_key] = opt_text
                        cleaned_options.append(strip_text(opt))
                    cleaned_field = strip_text(field)
                    cleaned_field["options"] = cleaned_options
                    cleaned_fields.append(cleaned_field)
                else:
                    cleaned_fields.append(strip_text(field))
            else:
                cleaned_fields.append(strip_text(field))
        cleaned["fields"] = cleaned_fields

    # Process inline spells in spells.list (when it's an array, i.e. inline)
    if "spells" in feature and isinstance(feature["spells"], dict):
        spells_def = feature["spells"]
        cleaned_spells = OrderedDict(spells_def)
        if "list" in spells_def and isinstance(spells_def["list"], list):
            cleaned_spell_list = []
            for spell in spells_def["list"]:
                sname = spell.get("name", "")
                if sname:
                    spell_key = f"{key}.spell.{sname}"
                    spell_text = extract_text(spell)
                    if spell_text:
                        locale_map[spell_key] = spell_text
                cleaned_spell_list.append(strip_text(spell))
            cleaned_spells["list"] = cleaned_spell_list
        cleaned["spells"] = cleaned_spells

    return cleaned


def process_class(data):
    """Process a class definition, returning (data_only, locale_map)."""
    locale_map = OrderedDict()

    # Root text
    root_text = extract_text(data)
    if root_text:
        locale_map[""] = root_text

    cleaned = strip_text(data)

    # Features
    if "features" in data:
        cleaned["features"] = [
            process_feature(f, "", locale_map) for f in data["features"]
        ]

    # Subclasses
    if "subclasses" in data:
        cleaned_subclasses = []
        for sc in data["subclasses"]:
            sc_name = sc.get("name", "")
            if sc_name:
                sc_key = f"subclass.{sc_name}"
                sc_text = extract_text(sc)
                if sc_text:
                    locale_map[sc_key] = sc_text

            cleaned_sc = strip_text(sc)

            # Subclass features
            if "features" in sc:
                cleaned_sc["features"] = [
                    process_feature(
                        f, f"subclass.{sc_name}.feature" if sc_name else "feature", locale_map
                    )
                    for f in sc["features"]
                ]
                # Fix: the prefix for subclass features should produce keys like
                # "subclass.X.feature.Y", so we need to adjust
                # Actually process_feature uses prefix.name, so with prefix="subclass.X.feature"
                # it produces "subclass.X.feature.Y" — correct!

            # Subclass levels (keep as-is)
            if "levels" in sc:
                cleaned_sc["levels"] = sc["levels"]

            cleaned_subclasses.append(cleaned_sc)
        cleaned["subclasses"] = cleaned_subclasses

    # Levels (keep as-is)
    if "levels" in data:
        cleaned["levels"] = data["levels"]

    return cleaned, locale_map


def process_race(data):
    """Process a race definition."""
    locale_map = OrderedDict()

    root_text = extract_text(data)
    if root_text:
        locale_map[""] = root_text

    cleaned = strip_text(data)

    # Traits
    if "traits" in data:
        cleaned_traits = []
        for trait in data["traits"]:
            tname = trait.get("name", "")
            if tname:
                trait_key = f"trait.{tname}"
                trait_text = extract_text(trait)
                if trait_text:
                    locale_map[trait_key] = trait_text
            cleaned_traits.append(strip_text(trait))
        cleaned["traits"] = cleaned_traits

    # Features
    if "features" in data:
        cleaned["features"] = [
            process_feature(f, "", locale_map) for f in data["features"]
        ]

    return cleaned, locale_map


def process_background(data):
    """Process a background definition."""
    locale_map = OrderedDict()

    root_text = extract_text(data)
    if root_text:
        locale_map[""] = root_text

    cleaned = strip_text(data)

    # Features
    if "features" in data:
        cleaned["features"] = [
            process_feature(f, "", locale_map) for f in data["features"]
        ]

    return cleaned, locale_map


def process_spell_list(data):
    """Process a spell list (array of spells)."""
    locale_map = OrderedDict()
    cleaned = []

    for spell in data:
        sname = spell.get("name", "")
        if sname:
            spell_text = extract_text(spell)
            if spell_text:
                locale_map[sname] = spell_text
        cleaned.append(strip_text(spell))

    return cleaned, locale_map


def process_index(data):
    """Process index.json."""
    locale_map = OrderedDict()
    cleaned = OrderedDict()

    for category in ["classes", "races", "backgrounds", "spells"]:
        if category not in data:
            continue
        cleaned_entries = []
        # Map category to singular for key prefix
        prefix_map = {
            "classes": "class",
            "races": "race",
            "backgrounds": "background",
            "spells": "spell",
        }
        prefix = prefix_map[category]
        for entry in data[category]:
            ename = entry.get("name", "")
            if ename:
                entry_text = extract_text(entry)
                if entry_text:
                    locale_map[f"{prefix}.{ename}"] = entry_text
            cleaned_entries.append(strip_text(entry))
        cleaned[category] = cleaned_entries

    return cleaned, locale_map


def process_effects(data):
    """Process effects.json."""
    locale_map = OrderedDict()
    cleaned = []

    for effect in data:
        ename = effect.get("name", "")
        if ename:
            effect_text = extract_text(effect)
            if effect_text:
                locale_map[ename] = effect_text
        cleaned.append(strip_text(effect))

    return cleaned, locale_map


def write_json(path, data):
    """Write JSON file with consistent formatting."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
        f.write("\n")


def process_directory(category, processor):
    """Process a directory of files (classes, races, backgrounds, spells)."""
    # Use 'en' as the reference for data files
    en_dir = ROOT / "en" / category
    if not en_dir.exists():
        return

    data_dir = ROOT / "data" / category
    data_dir.mkdir(parents=True, exist_ok=True)

    for json_file in sorted(en_dir.glob("*.json")):
        filename = json_file.name

        # Process each locale
        data_written = False
        for locale in LOCALES:
            locale_file = ROOT / locale / category / filename
            if not locale_file.exists():
                print(f"  SKIP {locale}/{category}/{filename} (not found)")
                continue

            with open(locale_file, encoding="utf-8") as f:
                data = json.load(f)

            cleaned, locale_map = processor(data)

            # Write data file only once (from first locale — they should be identical)
            if not data_written:
                write_json(data_dir / filename, cleaned)
                data_written = True
                print(f"  DATA data/{category}/{filename}")

            # Write locale file
            locale_out = ROOT / locale / category / filename
            write_json(locale_out, locale_map)
            print(f"  I18N {locale}/{category}/{filename} ({len(locale_map)} keys)")


def process_single_file(filename, processor):
    """Process a single file (index.json, effects.json)."""
    data_written = False
    for locale in LOCALES:
        locale_file = ROOT / locale / filename
        if not locale_file.exists():
            print(f"  SKIP {locale}/{filename} (not found)")
            continue

        with open(locale_file, encoding="utf-8") as f:
            data = json.load(f)

        cleaned, locale_map = processor(data)

        if not data_written:
            write_json(ROOT / "data" / filename, cleaned)
            data_written = True
            print(f"  DATA data/{filename}")

        write_json(ROOT / locale / filename, locale_map)
        print(f"  I18N {locale}/{filename} ({len(locale_map)} keys)")


def main():
    print("Splitting data files...")
    print()

    print("[classes]")
    process_directory("classes", process_class)
    print()

    print("[races]")
    process_directory("races", process_race)
    print()

    print("[backgrounds]")
    process_directory("backgrounds", process_background)
    print()

    print("[spells]")
    process_directory("spells", process_spell_list)
    print()

    print("[index]")
    process_single_file("index.json", process_index)
    print()

    print("[effects]")
    process_single_file("effects.json", process_effects)
    print()

    print("Done!")


if __name__ == "__main__":
    main()
