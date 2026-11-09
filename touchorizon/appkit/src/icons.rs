//! An item's icon for the Touch Bar, from the game's prepared resources: the item model's
//! `layer0`, or for a block item the face its block model shows best. Flat pixels only (no 3D
//! block render), which reads well at Touch Bar size.
use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

/// RGBA pixels, not premultiplied, rows top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The texture keys of a block model, the most telling face first.
const BLOCK_FACES: [&str; 12] = [
    "layer0", "all", "side", "front", "texture", "cross", "plant", "top", "end", "wool", "pattern",
    "particle",
];

/// `minecraft:block/stone` (or `block/stone`) as (namespace, path).
fn split(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

/// A model's textures merged down its parents (a child's keys win), at most 12 levels.
fn model_textures(root: &Path, model: &str) -> serde_json::Map<String, Value> {
    let mut merged = serde_json::Map::new();
    let mut next = Some(model.to_owned());
    for _ in 0..12 {
        let Some(name) = next.take() else { break };
        let (ns, path) = split(&name);
        let file = root.join(format!("assets/{ns}/models/{path}.json"));
        let Some(json) = read_json(&file) else { break };
        if let Some(Value::Object(textures)) = json.get("textures") {
            for (k, v) in textures {
                merged.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        next = json.get("parent").and_then(Value::as_str).map(str::to_owned);
    }
    merged
}

/// A texture reference, `#side` indirections followed.
fn resolve<'a>(textures: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a str> {
    let mut v = textures.get(key)?.as_str()?;
    for _ in 0..8 {
        match v.strip_prefix('#') {
            Some(k) => v = textures.get(k)?.as_str()?,
            None => return Some(v),
        }
    }
    None
}

fn texture_file(root: &Path, texture: &str) -> PathBuf {
    let (ns, path) = split(texture);
    root.join(format!("assets/{ns}/textures/{path}.png"))
}

/// The texture file that stands for `item` (`minecraft:oak_planks`).
pub fn texture_for(root: &Path, item: &str) -> Option<PathBuf> {
    let (ns, name) = split(item);
    let textures = model_textures(root, &format!("{ns}:item/{name}"));
    let mut candidates: Vec<PathBuf> = BLOCK_FACES
        .iter()
        .filter_map(|k| resolve(&textures, k))
        .map(|t| texture_file(root, t))
        .collect();
    // Models the items/ definitions build in code (beds, chests, shulker boxes...) have no
    // textures of their own: the plain item, then the block's faces by name.
    for path in [
        format!("item/{name}"),
        format!("block/{name}"),
        format!("block/{name}_front"),
        format!("block/{name}_side"),
        format!("block/{name}_top"),
    ] {
        candidates.push(root.join(format!("assets/{ns}/textures/{path}.png")));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Decodes a PNG to RGBA; an animated strip (taller than wide) keeps its first frame.
pub fn decode(path: &Path) -> Option<Icon> {
    let bytes = fs::read(path).ok()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width, info.height);
    let px = (w * h) as usize;
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf[..px * 4].to_vec(),
        png::ColorType::Rgb => buf[..px * 3]
            .chunks_exact(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf[..px * 2]
            .chunks_exact(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => buf[..px].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    let frame = h.min(w);
    Some(Icon {
        width: w,
        height: frame,
        rgba: rgba[..(w * frame * 4) as usize].to_vec(),
    })
}

/// Grey textures the game tints by biome or colour: a plausible default tint.
fn tint_for(item: &str) -> Option<[u8; 3]> {
    let name = split(item).1;
    let foliage = [0x48, 0xB5, 0x18];
    if name == "birch_leaves" {
        return Some([0x80, 0xA7, 0x55]);
    }
    if name == "spruce_leaves" {
        return Some([0x61, 0x99, 0x61]);
    }
    let tinted = ["leaves", "grass", "fern", "vine", "lily_pad", "sugar_cane"];
    let skip = ["cherry", "azalea", "pale_oak", "dried", "seagrass"];
    (tinted.iter().any(|t| name.contains(t)) && !skip.iter().any(|s| name.contains(s)))
        .then_some(foliage)
}

fn is_grey(icon: &Icon) -> bool {
    icon.rgba
        .chunks_exact(4)
        .filter(|p| p[3] > 0)
        .all(|p| p[0].abs_diff(p[1]) < 6 && p[1].abs_diff(p[2]) < 6)
}

/// The icon of `item`, or None when the resources have nothing for it.
pub fn load(root: &Path, item: &str) -> Option<Icon> {
    let mut icon = decode(&texture_for(root, item)?)?;
    if let Some([r, g, b]) = tint_for(item)
        && is_grey(&icon)
    {
        for p in icon.rgba.chunks_exact_mut(4) {
            p[0] = (u16::from(p[0]) * u16::from(r) / 255) as u8;
            p[1] = (u16::from(p[1]) * u16::from(g) / 255) as u8;
            p[2] = (u16::from(p[2]) * u16::from(b) / 255) as u8;
        }
    }
    Some(icon)
}

/// `minecraft:oak_planks` as `Oak Planks`, for an item with no icon.
pub fn pretty_name(item: &str) -> String {
    split(item)
        .1
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, body: &[u8]) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn png_bytes(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(rgba).unwrap();
        out
    }

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("touchorizon-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    #[test]
    fn names_read_like_the_game() {
        assert_eq!(pretty_name("minecraft:oak_planks"), "Oak Planks");
        assert_eq!(pretty_name("diamond"), "Diamond");
    }

    #[test]
    fn follows_item_models_to_block_faces() {
        let root = temp_root("models");
        write(&root, "assets/minecraft/models/item/stone.json", br#"{"parent":"minecraft:block/stone"}"#);
        write(
            &root,
            "assets/minecraft/models/block/stone.json",
            br#"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"#,
        );
        write(
            &root,
            "assets/minecraft/models/block/cube_all.json",
            br##"{"textures":{"particle":"#all"}}"##,
        );
        write(&root, "assets/minecraft/textures/block/stone.png", &png_bytes(1, 1, &[1, 2, 3, 255]));
        assert_eq!(
            texture_for(&root, "minecraft:stone"),
            Some(root.join("assets/minecraft/textures/block/stone.png"))
        );
        // No model at all: the item texture by name.
        write(&root, "assets/minecraft/textures/item/stick.png", &png_bytes(1, 1, &[0; 4]));
        assert_eq!(
            texture_for(&root, "minecraft:stick"),
            Some(root.join("assets/minecraft/textures/item/stick.png"))
        );
        assert_eq!(texture_for(&root, "minecraft:nothing"), None);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn animated_strips_keep_the_first_frame_and_leaves_turn_green() {
        let root = temp_root("decode");
        let strip: Vec<u8> = [[200, 200, 200, 255], [9, 9, 9, 255]].concat();
        write(&root, "assets/minecraft/textures/block/oak_leaves.png", &png_bytes(1, 2, &strip));
        let icon = load(&root, "minecraft:oak_leaves").unwrap();
        assert_eq!((icon.width, icon.height), (1, 1));
        assert_eq!(icon.rgba, vec![0x38, 0x8D, 0x12, 255]);
        let _ = fs::remove_dir_all(&root);
    }
}
