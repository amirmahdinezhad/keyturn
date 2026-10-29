//! Verify updater artifacts with the same Minisign implementation as Tauri.
use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: verify_update <archive> <signature-file>".into());
    }
    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))?;
    let key_text = String::from_utf8(STANDARD.decode(
        config["plugins"]["updater"]["pubkey"].as_str().ok_or("missing public key")?,
    )?)?;
    let signature_text = String::from_utf8(STANDARD.decode(fs::read_to_string(&args[1])?.trim())?)?;
    let public_key = PublicKey::decode(&key_text)?;
    let signature = Signature::decode(&signature_text)?;
    public_key.verify(&fs::read(&args[0])?, &signature, true)?;
    println!("Updater signature verified.");
    Ok(())
}
