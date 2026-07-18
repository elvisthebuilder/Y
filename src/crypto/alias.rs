use rand::Rng;

const ADJECTIVES: &[&str] = &[
    "silent", "dark", "neon", "phantom", "ghost", "shadow", "void",
    "cyber", "toxic", "frozen", "burning", "hidden", "lost", "iron",
    "lunar", "solar", "deep", "wild", "bitter", "hollow", "broken",
    "chrome", "rust", "static", "zero", "null", "rogue", "feral",
    "ancient", "blind", "cold", "dead", "electric", "fatal", "grim",
];

const NOUNS: &[&str] = &[
    "fox", "wolf", "crow", "viper", "hawk", "shark", "spider",
    "orbit", "pulse", "signal", "wraith", "spectre", "cipher",
    "daemon", "node", "glitch", "storm", "blade", "echo", "drift",
    "spark", "flame", "frost", "thorn", "root", "core", "shell",
    "byte", "flux", "haze", "moss", "raven", "skull", "surge",
];

pub fn generate_alias() -> String {
    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.gen_range(0..NOUNS.len())];
    format!("{}-{}", adj, noun)
}

pub fn short_address(address: &str) -> String {
    let stripped = address.strip_prefix("root:").unwrap_or(address);
    stripped.chars().take(4).collect()
}

pub fn display_handle(alias: &str, address: &str) -> String {
    format!("{}#{}", alias, short_address(address))
}
