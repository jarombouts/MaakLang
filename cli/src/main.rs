//! Dev CLI / golden-test harness. Runs a Schildpad program, renders the framebuffer as ASCII,
//! and prints the event trace + any Tier-2 error. Not a host app — just a way to poke the core.
//!
//!   schildpad [FILE.maak]      run a file (or a built-in demo if omitted)
//!   schildpad --trace FILE     also dump the event stream

use std::env;
use std::fs;

use schildpad_core::command::DrawOp;
use schildpad_core::{Engine, Event};

const DEMO: &str = "\
maak pietje schildpad
maak punt draairichting = 144
penomhoog pietje
vooruit 60 pietje
draai rechts pietje
vooruit 90 pietje
draai links pietje
penomlaag pietje
pen rood pietje
herhaal 5
  vooruit 150 pietje
  draai punt pietje
";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut trace = false;
    let mut path: Option<String> = None;
    for a in &args {
        if a == "--trace" {
            trace = true;
        } else {
            path = Some(a.clone());
        }
    }

    let src = match &path {
        Some(p) => fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("kan {p} niet lezen: {e}");
            std::process::exit(1);
        }),
        None => DEMO.to_string(),
    };

    // a small canvas to visualise plots (logical 320x240 by default → downsample for the terminal)
    let cols = 40u16;
    let rows = 30u16;
    let (w, h) = (cols as usize * 8, rows as usize * 8);
    let mut canvas = vec![false; w * h];

    let mut engine = Engine::new();
    let mut events = engine.load(&src);
    let mut guard = 0;
    while !engine.done() && guard < 1_000_000 {
        events.extend(engine.step());
        guard += 1;
    }

    let mut errored = None;
    let (mut plots, mut audio) = (0usize, 0usize);
    for ev in &events {
        match ev {
            Event::Draw(DrawOp::Plot { x, y, .. }) => {
                plots += 1;
                let (x, y) = (*x as usize, *y as usize);
                if x < w && y < h {
                    canvas[y * w + x] = true;
                }
            }
            Event::Draw(DrawOp::Clear { .. }) => canvas.iter_mut().for_each(|p| *p = false),
            Event::Audio(_) => audio += 1,
            Event::Error(e) => errored = Some(e.clone()),
            _ => {}
        }
    }

    render_ascii(&canvas, w, h);
    println!("\nplots: {plots}   audio events: {audio}   total events: {}", events.len());

    if let Some(e) = errored {
        println!("\nFOUT op regel {}:\n  {}", e.line, e.render_nl());
    } else {
        println!("\nklaar — geen fouten.");
    }

    if trace {
        println!("\n--- trace ---");
        for ev in &events {
            println!("  {ev:?}");
        }
    }
}

/// Downsample the logical canvas to ~80 columns and print as ASCII.
fn render_ascii(canvas: &[bool], w: usize, h: usize) {
    let target_w = 80usize.min(w);
    let sx = (w + target_w - 1) / target_w;
    let sy = sx * 2; // characters are ~2x taller than wide
    let mut y = 0;
    while y < h {
        let mut line = String::new();
        let mut x = 0;
        while x < w {
            let mut on = false;
            'blk: for dy in 0..sy {
                for dx in 0..sx {
                    let (px, py) = (x + dx, y + dy);
                    if px < w && py < h && canvas[py * w + px] {
                        on = true;
                        break 'blk;
                    }
                }
            }
            line.push(if on { '#' } else { ' ' });
            x += sx;
        }
        println!("{}", line.trim_end());
        y += sy;
    }
}
