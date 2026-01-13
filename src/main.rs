use rand::Rng;
use regex::Regex;
use serde_json::{Value, json};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, env, fs, path::Path};
use tokio::time::sleep;

const OFFSET: isize = -3;
const SHINY_TEXT: &str = "\u{2605} Shiny! \u{2605}";
const SHINY_PROBABILITY: i32 = 1;

struct PokeFastFetch {
    cached_path: String,
    ff_config_path: String,
    color_fmt: String,
    ff_lines: usize,
}

fn module(t: &str, key: &str, color: &str) -> Value {
    json!({
        "type": t,
        "key": key,
        "keyColor": color,
        "valueColor": color
    })
}

impl PokeFastFetch {
    fn new(cached_path: String, ff_config_path: String) -> anyhow::Result<Self> {
        let (color_fmt, ff_lines) = Self::extract_colors(&cached_path)?;
        Ok(Self {
            cached_path,
            ff_config_path,
            color_fmt,
            ff_lines,
        })
    }

    fn quantize_color((r, g, b): (u8, u8, u8), step: u8) -> (u8, u8, u8) {
        ((r / step) * step, (g / step) * step, (b / step) * step)
    }

    fn extract_colors(path: &str) -> anyhow::Result<(String, usize)> {
        let pokemon = fs::read_to_string(path)?;
        let pokemon_lines = pokemon.lines().count();
        let ff_lines = (pokemon_lines as isize + OFFSET).max(0) as usize;

        let re = Regex::new(r"(?:38|48);2;(\d{1,3});(\d{1,3});(\d{1,3})")?;

        let mut counts: HashMap<(u8, u8, u8), usize> = HashMap::new();

        for cap in re.captures_iter(&pokemon) {
            let r: u8 = cap[1].parse()?;
            let g: u8 = cap[2].parse()?;
            let b: u8 = cap[3].parse()?;

            let dark = r < 90 && g < 90 && b < 90;
            let light = r > 180 && g > 180 && b > 180;
            if dark || light {
                continue;
            }

            let q = Self::quantize_color((r, g, b), 8);
            *counts.entry(q).or_insert(0) += 1;
        }

        let ((r, g, b), _) = counts
            .into_iter()
            .max_by_key(|(_, v)| *v)
            .ok_or_else(|| anyhow::anyhow!("ANSI parsing failed"))?;

        Ok((format!("38;2;{};{};{}", r, g, b), ff_lines))
    }

    fn run(&self, pokemon_display: String) -> anyhow::Result<()> {
        let mut config: Value = serde_json::from_str(&fs::read_to_string(&self.ff_config_path)?)?;

        config["logo"] = json!({
            "type": "command-raw",
            "source": format!("cat {}", self.cached_path),
            "padding": {
                "top": 2
            }
        });

        if !config.get("display").is_some() {
            config["display"] = json!({});
        }
        if !config["display"].get("color").is_some() {
            config["display"]["color"] = json!({});
        }

        config["display"]["color"]["title"] = json!(self.color_fmt);
        config["display"]["color"]["keys"] = json!(self.color_fmt);

        let mut modules: Vec<Value> = vec![];

        modules.push(json!("title"));

        modules.push(json!("separator"));

        modules.push(module("os", "os    ", &self.color_fmt));
        modules.push(module("kernel", "kernel", &self.color_fmt));
        modules.push(module("uptime", "uptime", &self.color_fmt));
        modules.push(module("processes", "proc  ", &self.color_fmt));
        modules.push(module("packages", "pkgs  ", &self.color_fmt));
        modules.push(module("shell", "shell ", &self.color_fmt));
        modules.push(module("monitor", "mon   ", &self.color_fmt));
        modules.push(module("terminal", "term  ", &self.color_fmt));
        modules.push(json!({
            "type": "cpu",
            "key": "cpu   ",
            "keyColor": self.color_fmt,
            "valueColor": self.color_fmt,
            "showPeCoreCount": false,
            "temp": true
        }));
        modules.push(json!({
            "type": "cpuusage",
            "key": "usage ",
            "keyColor": self.color_fmt,
            "valueColor": self.color_fmt
        }));
        modules.push(json!({
            "type": "gpu",
            "key": "gpu   ",
            "keyColor": self.color_fmt,
            "valueColor": self.color_fmt,
            "driverSpecific": true,
            "temp": true
        }));
        modules.push(module("memory", "memory", &self.color_fmt));
        modules.push(module("disk", "disk  ", &self.color_fmt));
        modules.push(module("media", "media ", &self.color_fmt));
        modules.push(module("datetime", "time ", &self.color_fmt));
        modules.push(module("version", "ver   ", &self.color_fmt));
        modules.push(json!("separator"));

        modules.push(json!({
            "type": "custom",
            "key": "pokemon",
            "format": pokemon_display,
            "keyColor": self.color_fmt,
            "valueColor": self.color_fmt
        }));

        modules.push(json!("break"));
        modules.push(json!("colors"));

        config["modules"] = Value::Array(modules);

        fs::write(&self.ff_config_path, serde_json::to_string_pretty(&config)?)?;

        Ok(())
    }
}

fn extract_types(pokemon: &Value) -> Vec<String> {
    pokemon["types"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|entry| entry["type"]["name"].as_str().map(|s| s.to_string()))
        .collect()
}

fn ansi_bg(color: u8) -> String {
    format!("\x1b[48;5;{}m", color)
}

fn ansi_fg(color: u8) -> String {
    format!("\x1b[38;5;{}m", color)
}

fn ansi_reset() -> &'static str {
    "\x1b[0m"
}

fn foreground_for_bg(bg: u8) -> u8 {
    match bg {
        // Dark backgrounds → white text
        1 | 5 | 8 | 21 | 99 | 236 | 55 => 255,
        // Default → black text
        _ => 232,
    }
}

fn pokemon_type_color(pokemon_type: &str) -> u8 {
    match pokemon_type {
        "normal" => 101,
        "fire" => 202,
        "water" => 31,
        "electric" => 226,
        "grass" => 76,
        "ice" => 81,
        "fighting" => 124,
        "poison" => 127,
        "ground" => 178,
        "flying" => 98,
        "psychic" => 170,
        "bug" => 142,
        "rock" => 101,
        "ghost" => 55,
        "dragon" => 21,
        "dark" => 236,
        "steel" => 247,
        "fairy" => 219,
        _ => 0,
    }
}

fn all_pokemon_types() -> Vec<String> {
    vec![
        "normal".to_string(),
        "fire".to_string(),
        "water".to_string(),
        "electric".to_string(),
        "grass".to_string(),
        "ice".to_string(),
        "fighting".to_string(),
        "poison".to_string(),
        "ground".to_string(),
        "flying".to_string(),
        "psychic".to_string(),
        "bug".to_string(),
        "rock".to_string(),
        "ghost".to_string(),
        "dragon".to_string(),
        "dark".to_string(),
        "steel".to_string(),
        "fairy".to_string(),
    ]
}

fn create_text_badge(text: &str, bg_color: u8, bold: bool) -> String {
    let fg_color = foreground_for_bg(bg_color);

    let fg = ansi_fg(fg_color);
    let bg = ansi_bg(bg_color);
    let bold_code = if bold { "\x1b[1m" } else { "" };

    format!(
        "{}{}{} {} {}{}",
        bold_code,
        fg,
        bg,
        text,
        ansi_reset(),
        ansi_reset()
    )
}

fn get_type_badges(types: &[String]) -> String {
    types
        .iter()
        .map(|t| {
            let color = pokemon_type_color(t);
            create_text_badge(&t.to_uppercase(), color, false)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn format_pokemon_display(name: &str, types: &[String], is_shiny: bool) -> String {
    // 1️⃣ Capitalize Pokémon name
    let name = capitalize(name);

    // 2️⃣ Stylize name as a badge (optional color: 15 = white background)
    let name_badge = create_text_badge(&name, 15, true);

    // 3️⃣ Shiny marker
    let shiny_badge = if is_shiny {
        // Yellow background for shiny, black foreground
        create_text_badge("★ Shiny! ★", 220, true)
    } else {
        "".to_string()
    };

    // 4️⃣ Type badges
    let type_badges = get_type_badges(types);

    // 5️⃣ Combine all in a single line
    if is_shiny {
        format!("{} {} {}", name_badge, shiny_badge, type_badges)
    } else {
        format!("{} {}", name_badge, type_badges)
    }
}

fn strip_pokemon_form(name: &str) -> &str {
    name.split('-').next().unwrap_or(name)
}

fn change_invalid_names(pokemon_id: &i32, pokemon: &Value) -> String {
    match pokemon_id {
        29 => String::from("nidoran-f"),
        32 => String::from("nidoran-m"),
        122 => String::from("mr-mime"),
        386 => String::from("deoxys"),
        413 => String::from("wormadam"),
        487 => String::from("giratina"),
        492 => String::from("shaymin"),
        550 => String::from("basculin"),
        555 => String::from("darmanitan"),
        641 => String::from("tornadus"),
        642 => String::from("thundurus"),
        645 => String::from("landorus"),
        647 => String::from("keldeo"),
        648 => String::from("meloetta"),
        678 => String::from("meowstic"),
        681 => String::from("aegislash"),
        710 => String::from("pumpkaboo"),
        711 => String::from("gourgeist"),
        718 => String::from("zygarde"),
        741 => String::from("oricorio"),
        745 => String::from("lycanroc"),
        746 => String::from("wishiwashi"),
        774 => String::from("minior"),
        778 => String::from("mimikyu"),
        849 => String::from("toxtricity"),
        875 => String::from("eiscue"),
        876 => String::from("indeedee"),
        877 => String::from("morpeko"),
        892 => String::from("urshifu"),
        902 => String::from("basculegion"),
        _ => String::from(pokemon["name"].as_str().unwrap()),
    }
}

fn try_generate_colorscript(pokemon_name: &str, shiny: bool) -> bool {
    let mut cmd = Command::new("pokemon-colorscripts");

    cmd.arg("-n").arg(pokemon_name).arg("--no-title");

    if shiny {
        cmd.arg("--shiny");
    }

    match cmd.output() {
        Ok(output) => output.status.success() && !output.stdout.is_empty(),
        Err(_) => false,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rand_value = rand::rng().random_range(1..=4);
    let is_shiny = rand_value == SHINY_PROBABILITY;

    let client = reqwest::Client::new();

    let random_id = rand::rng().random_range(1..=904);

    let url = format!("https://pokeapi.co/api/v2/pokemon/{}", random_id);

    let pokemon: Value = client.get(url).send().await?.json().await?;

    let pokemon_name = strip_pokemon_form(pokemon["name"].as_str().unwrap());

    let display = format_pokemon_display(&*pokemon_name, &extract_types(&pokemon), is_shiny);

    // --- 1️⃣ Get paths ---
    let home = env::var("HOME")?;
    let cached = format!("{}/.cache/pokemon.txt", home);
    let ff_cfg = format!("{}/.config/fastfetch/config.jsonc", home);

    let shiny_arg = if is_shiny { "-s" } else { "" };

    // --- 3️⃣ Generate Pokémon ASCII ---
    let cmd = format!(
        "pokemon-colorscripts -n {} {} --no-title > {}",
        pokemon_name, shiny_arg, cached
    );
    let status = Command::new("sh").arg("-c").arg(cmd).status()?;
    if !status.success() {
        eprintln!("Failed to generate Pokémon ASCII!");
    }

    // --- 4️⃣ Update Fastfetch config with Pokémon colors ---
    PokeFastFetch::new(cached.clone(), ff_cfg.clone())?
        .run(display)?;

    // --- 5️⃣ Run fastfetch ---
    let status = Command::new("fastfetch").status()?;
    if !status.success() {
        eprintln!("Failed to run fastfetch!");
    }

    Ok(())
}
