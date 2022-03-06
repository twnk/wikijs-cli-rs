use clap::{ArgEnum, Args, Parser, Subcommand};
use enable_ansi_support;
use owo_colors::{OwoColorize, Stream, Style};
use owo_colors::colors::*;
use console::{Term, Emoji};
use dialoguer::Confirm;

use human_panic;
use anyhow::{Result, bail};

use itertools::Itertools;

mod lib;
mod creds;
use lib::Wiki;

/// A very simple utility for bulk operations on Wiki pages.
#[derive(Debug, Parser)]
#[clap(name = "wiki", version = "0.1.0", author = "Angel~👼")]
pub struct App {
    #[clap(flatten)]
    global_opts: GlobalOpts,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List wiki pages by path prefix
    List {
        /// Path prefix   
        path: String,

        // Filter by Tags
        #[clap(long, short = 't')]
        tags: Option<Vec<String>>
    },
    /// Move wiki pages to a new path
    Move {
        /// Path prefix
        path: String,

        /// Destination to replace prefix
        #[clap(long, short = 'd')]
        destination: String,

        // Filter by Tags
        #[clap(long, short = 't')]
        tags: Option<Vec<String>>,

        /// Tags to add
        #[clap(long)]
        add_tags: Option<Vec<String>>
    }
}


#[derive(Debug, Args)]
struct GlobalOpts {
    /// Color
    #[clap(long, arg_enum, global = true, default_value_t = Color::Auto)]
    color: Color,

    /// Verbosity level (can be specified multiple times)
    #[clap(long, short, global = true, parse(from_occurrences))]
    verbose: usize,
    //... other global options
}

#[derive(Clone, Copy, Debug, ArgEnum)]
enum Color {
    Always,
    Auto,
    Never,
}

impl Color {
    fn init(self) {
        // Set a supports-color override based on the variable passed in.
        match self {
            Color::Always => owo_colors::set_override(true),
            Color::Auto => { }
            Color::Never => owo_colors::set_override(false),
        }
    }
}

struct Styles {
    scaffold: Style,
    message: Style,
    user: Style,
    output: Style
}


#[tokio::main]
async fn main() -> Result<()> {
    // Make panic message more useful
    human_panic::setup_panic!(Metadata {
        name: env!("CARGO_PKG_NAME").into(),
        version: env!("CARGO_PKG_VERSION").into(),
        authors: "Angel~ <aj@redfightback.org> / @aj on Comms".into(),
        homepage: "Attach to a DM on Comms instead of email please <33".into(),
    });

    let app = App::parse();
    
    // Windows 10 Terminals can do ANSI colors with your help! 
    match enable_ansi_support::enable_ansi_support() {
        Ok(_) => app.global_opts.color.init(),
        Err(_) => {}
    }
    
    println!("{:?}", &app);
    let term = Term::stdout();

    let styles = Styles{
        scaffold: Style::new().bright_white().on_black(),
        message: Style::new().blue().on_black(),
        user: Style::new().bg::<xterm::ElectricIndigo>().fg::<xterm::Copperfield>().underline(),
        output: Style::new().fg::<xterm::ElectricIndigo>().on_bright_white()

    };

    term.write_line(&format!(
        "{} {}  {}.",
        "[1/3]".if_supports_color(
            Stream::Stdout, |text| text.style(styles.scaffold)),
        Emoji("☎️", ""),
        "Preparing to connect to the Wiki".if_supports_color(
            Stream::Stdout, |text| text.style(styles.message))
    ))?;

    let wiki = Wiki::new(creds::BEARER);

    match app.command {
        Command::List { path, tags } => {
            term.write_line(&format!(
                "{} {}  {} {} {}.",
                "[2/3]".if_supports_color(
                    Stream::Stdout, |text| text.style(styles.scaffold)),
                Emoji("🔍", ""),
                "Finding all pages beginning with".if_supports_color(
                    Stream::Stdout, |text| text.style(styles.message)),
                &path.if_supports_color(
                    Stream::Stdout, |text| text.style(styles.user)),
                match &tags {
                    Some(tags) => format!("{} {}",
                        "which have the tags:".if_supports_color(
                            Stream::Stdout, |text| text.style(styles.message)),
                        &tags.join(", ").if_supports_color(
                            Stream::Stdout, |text| text.style(styles.user))
                    ),
                    None => String::new()
                }
            ))?;
            let trim = path.len(); // keep for string trimming later

            let pages = wiki.list_pages(&path, tags).await?;

            term.write_line(&format!(
                "{} {}  {} {} {} {}.",
                "[3/3]".if_supports_color(
                    Stream::Stdout, |text| text.style(styles.scaffold)),
                Emoji("📝", ""),
                "Formatting".if_supports_color(
                    Stream::Stdout, |text| text.style(styles.message)),
                &pages.pages.len().if_supports_color(
                    Stream::Stdout, |text| text.style(styles.output)),
                "matching pages".if_supports_color(
                    Stream::Stdout, |text| text.style(styles.message)),
                match app.global_opts.verbose {
                    0 => String::new(),
                    _ => format!("{} {} {}",
                        "out of ".if_supports_color(
                            Stream::Stdout, |text| text.style(styles.message)),
                        pages.pages_returned.if_supports_color(
                            Stream::Stdout, |text| text.style(styles.output)),
                        "returned by wiki".if_supports_color(
                            Stream::Stdout, |text| text.style(styles.message))
                        )
                }
            ))?;

            let header = "ID\tPath\tTitle\tTags".if_supports_color(
                Stream::Stdout, |text| text.style(styles.message));

            
            let null_title = "[Untitled]";

            let max_path = match pages.pages.iter().map(|p| p.path.len()).max() {
                Some(s) => s - trim,
                None => 50
            };
            
            let lines = pages.pages.into_iter()
                .map(|p|{
                    format!(
                        "{}\t{}\t{} ({})",
                        p.id,
                        console::pad_str(&p.path[trim..], max_path, console::Alignment::Left, Some("…")),
                        match p.title {
                            Some(t) => t,
                            None => null_title.to_string()
                        },
                        match p.tags {
                            Some(ts) => ts.into_iter().flatten().join(", "),
                            None => String::new()

                        }
                    )
                })
                .join("\n");

            // term.clear_last_lines(3)?;
            term.write_line(&lines)?;
            

        },
        Command::Move { path, destination, tags, add_tags} => {
            term.write_line(&format!(
                "[2/3] {}  Finding all pages beginning with {} {}.",
                Emoji("🔍", ""),
                &path,
                match &tags {
                    Some(tags) => format!("which have the tags: {}",
                        &tags.join(", ")
                    ),
                    None => String::new()
                }
            ))?;
            let trim = path.len(); // keep for string trimming later

            let lib::ListPages{pages, pages_returned} = wiki.list_pages(&path, tags).await?;

            term.write_line(&format!(
                "[3/3] {}  Formatting {} matching pages {}.",
                Emoji("📝", ""),
                &pages.len(),
                match app.global_opts.verbose {
                    0 => String::new(),
                    _ => format!("out of {} returned by wiki", pages_returned)
                }
            ))?;

            let header = "ID\tPath\tTitle\tTags";

            let null_title = "[Untitled]";

            let max_path = match pages.iter().map(|p| p.path.len()).max() {
                Some(s) => s - trim,
                None => 50
            };
            
            let lines = pages.iter()
                .map(|p| -> String {
                    format!(
                        "{}\t{}\t{} ({})",
                        p.id,
                        console::pad_str(&p.path[trim..], max_path, console::Alignment::Left, Some("…")),
                        match &p.title {
                            Some(t) => t,
                            None => null_title
                        },
                        match &p.tags {
                            Some(ts) => ts.into_iter().flatten().join(", "),
                            None => String::new()

                        }
                    )
                })
                .join("\n");

            // term.clear_last_lines(3)?;
            term.write_line(&lines)?;

            term.write_line(&format!(
                "{} All of these pages will be relocated from {}… to {}…!", 
                Emoji("📎",""),
                &path,
                &destination
            ))?;


            let proceed = Confirm::new()
                .with_prompt("Are you sure you want to do this?")
                .interact_on(&Term::stderr())?;

            if !proceed {bail!("User was not sure they want to do this.")} // is it an error? 
            
            let private_pages = wiki.safety_check_private(pages.iter()).await;

            let check_private = match private_pages {
                Some(pgs) => {
                    term.write_line("The following pages you intend to move are marked as private:")?;
                    let lines = pgs
                        .map(|p| -> String {
                            format!(
                                "{}\t{}\t{} ({})",
                                p.id,
                                console::pad_str(&p.path[trim..], max_path, console::Alignment::Left, Some("…")),
                                match &p.title {
                                    Some(t) => t,
                                    None => null_title
                                },
                                match &p.tags {
                                    Some(ts) => ts.into_iter().flatten().join(", "),
                                    None => String::new()

                                }
                            )
                        })
                        .join("\n");
                    term.write_line(&lines)?;
                    true
                },
                None => false
            };

            if check_private {
                let proceed = Confirm::new()
                        .with_prompt("Moving private pages may change who can access them.\nAre you really sure you want to move private pages?")
                        .interact_on(&Term::stderr())?;

                if !proceed {bail!("User was not really sure they want to move private pages.")}
            }

            let tags = wiki.tag_pages(&pages, &path, &destination, add_tags).await?;

            match tags.failures {
                None => {
                    term.write_line(&format!(
                        "All pages have been tagged successfully, including a safety tag {}. Tags applied: {}.", 
                        tags.safety_tag,
                        tags.tags.join(", ")
                    ))?;
                }
                Some(fails) => {
                    term.write_line(&format!(
                        "{} failures occured during tagging. {} successes occured. Pages may be inconsistently tagged. Tags attempted: {}.", 
                        fails.len(),
                        tags.success_count,
                        tags.tags.join(", ")
                    ))?;
                    let blank = String::new();
                    term.write_line(&fails.iter().map(
                        |(f,pg)| format!(
                            "Code: {} Slug: {} Message: {}{}", 
                            f.error_code, 
                            f.slug, 
                            &f.message.as_ref().unwrap_or(&blank),
                            match pg {
                                Some(p) => format!("Page: {} {} ({})",p.path, p.title, p.tags.iter().filter_map(|t|match t {
                                    Some(tag) => Some(tag.tag.clone()),
                                    None => None
                                }).join(", ")),
                                None => String::new()
                            }
                        )).join("\n"))?;
                }
            }

            let proceed = Confirm::new()
                .with_prompt("Do you want to proceed to move pages?")
                .interact_on(&Term::stderr())?;

            if !proceed {bail!("User did not want to proceed.")} // is it an error? 

            
            
        }

    }

    
    // ;
    Ok(())
}
