//! Resolve name => gid assuming Google Fonts icon font input

use skrifa::{
    charmap::Charmap,
    raw::{
        tables::gsub::{ExtensionSubtable, LigatureSubstFormat1, SubstitutionLookup},
        FontRef, TableProvider,
    },
    GlyphId,
};

use crate::error::IconNameError;

fn resolve_ligature(
    liga: &LigatureSubstFormat1<'_>,
    text: &str,
    gids: &[GlyphId],
) -> Result<Option<GlyphId>, IconNameError> {
    let Some(first) = gids.first() else {
        return Err(IconNameError::NoGlyphIds(text.to_string()));
    };
    let coverage = liga.coverage().map_err(IconNameError::ReadError)?;
    let Some(set_index) = coverage.get(*first) else {
        return Ok(None);
    };
    let set = liga
        .ligature_sets()
        .get(set_index as usize)
        .map_err(IconNameError::ReadError)?;
    // Seek a ligature that matches glyphs 2..N of name
    // We don't care about speed
    let gids = &gids[1..];
    for liga in set.ligatures().iter() {
        let liga = liga.map_err(IconNameError::ReadError)?;
        if liga.component_count() as usize != gids.len() + 1 {
            continue;
        }
        if gids
            .iter()
            .zip(liga.component_glyph_ids())
            .all(|(gid, component)| *gid == component.get())
        {
            return Ok(Some(liga.ligature_glyph())); // We found it!
        }
    }
    Ok(None)
}

pub fn icon_name_to_gid(font: &FontRef, name: &str) -> Result<GlyphId, IconNameError> {
    let charmap = Charmap::new(font);
    let gids = name
        .chars()
        .map(|c| charmap.map(c).ok_or(IconNameError::UnmappedCharError(c)))
        .collect::<Result<Vec<_>, _>>()?;

    // Step 1: try to find a ligature that starts with our first gid
    let gsub = font.gsub().map_err(IconNameError::ReadError)?;
    let lookups = gsub.lookup_list().map_err(IconNameError::ReadError)?;
    for lookup in lookups.lookups().iter() {
        let lookup = lookup.map_err(IconNameError::ReadError)?;
        match lookup {
            SubstitutionLookup::Ligature(table) => {
                for liga in table.subtables().iter() {
                    let liga = liga.map_err(IconNameError::ReadError)?;
                    if let Some(gid) = resolve_ligature(&liga, name, &gids)? {
                        return Ok(gid);
                    }
                }
            }
            SubstitutionLookup::Extension(table) => {
                for lookup in table.subtables().iter() {
                    let ExtensionSubtable::Ligature(table) =
                        lookup.map_err(IconNameError::ReadError)?
                    else {
                        continue;
                    };
                    let table = table.extension().map_err(IconNameError::ReadError)?;

                    if let Some(gid) = resolve_ligature(&table, name, &gids)? {
                        return Ok(gid);
                    }
                }
            }
            _ => (),
        }
    }
    Err(IconNameError::NoLigature(name.to_string()))
}
