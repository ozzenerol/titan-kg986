mod device;
mod gui;
mod icon;
mod preview;
mod protocol;
mod widgets;

use device::Device;
use protocol::Config;
use std::time::Duration;

fn print_lighting(c: &Config) {
    let l = c.lighting();
    println!(
        "effect {} brightness {} speed {} {}",
        l.effect,
        l.brightness,
        l.speed,
        if l.colorful { "colorful" } else { "single" }
    );
}

fn diff(a: &Config, b: &Config) {
    for (i, (fa, fb)) in a.frames.iter().zip(b.frames.iter()).enumerate() {
        for j in 4..protocol::FRAME_LEN - 1 {
            if fa[j] != fb[j] {
                println!("  frag {i} byte {j}: {:02x} -> {:02x}", fa[j], fb[j]);
            }
        }
    }
}

fn parse_rgb(s: &str) -> Result<[u8; 3], String> {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).map_err(|_| format!("bad color {s}"))?;
    if s.len() != 6 {
        return Err(format!("bad color {s}"));
    }
    Ok([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).unwrap_or("status");
    if cmd == "icon" {
        let path = args.get(1).ok_or("usage: icon FILE.png")?;
        return icon::write_png(std::path::Path::new(path));
    }
    let mut dev = Device::open()?;
    match cmd {
        "status" => {
            let c = dev.read_config()?;
            println!("{}", dev.path.display());
            print_lighting(&c);
            if args.iter().any(|a| a == "--raw") {
                print!("{}", c.to_hex());
            }
        }
        "watch" => {
            let mut last = dev.read_config()?;
            print_lighting(&last);
            loop {
                std::thread::sleep(Duration::from_millis(700));
                let c = match dev.read_config() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("read failed: {e}");
                        continue;
                    }
                };
                if c != last {
                    print_lighting(&c);
                    diff(&last, &c);
                    last = c;
                }
            }
        }
        "backup" => {
            let c = dev.read_config()?;
            let path = args.get(1).ok_or("usage: backup FILE")?;
            std::fs::write(path, c.to_hex()).map_err(|e| e.to_string())?;
            println!("saved {path}");
        }
        "set" => {
            let num = |i: usize, name: &str| -> Result<u8, String> {
                args.get(i)
                    .ok_or(format!("usage: set EFFECT BRIGHTNESS SPEED colorful|single [RRGGBB]; missing {name}"))?
                    .parse()
                    .map_err(|_| format!("bad {name}"))
            };
            let want = protocol::Lighting {
                effect: num(1, "effect")?,
                brightness: num(2, "brightness")?,
                speed: num(3, "speed")?,
                colorful: args.get(4).map(String::as_str) != Some("single"),
            };
            let color = args.get(5).map(|s| parse_rgb(s)).transpose()?;
            let got = dev.apply(want, color)?;
            print_lighting(&got);
        }
        "restore" => {
            let path = args.get(1).ok_or("usage: restore FILE")?;
            let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
            let saved = Config::from_hex(&text)?;
            dev.write(&saved.write_frames(None))?;
            let got = dev.read_config()?;
            print_lighting(&got);
        }
        _ => return Err(format!("unknown command {cmd}")),
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = if args.is_empty() { gui::run() } else { run(&args) };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
