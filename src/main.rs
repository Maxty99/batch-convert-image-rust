use std::{
    env,
    ffi::OsString,
    fmt::Display,
    fs,
    num::NonZeroUsize,
    path::PathBuf,
    sync::{Arc, atomic::AtomicUsize},
    usize,
};

use anyhow::{Context, Error, Result, anyhow};
use clap::{
    Parser,
    builder::{NonEmptyStringValueParser, TypedValueParser, ValueParser, ValueParserFactory},
};
use image::{ImageFormat, ImageReader};
use indicatif::{MultiProgress, ParallelProgressIterator, ProgressBar, ProgressStyle};
use jwalk::{ClientState, DirEntry, Parallelism, WalkDirGeneric};
use rayon::prelude::*;

const HELP_TEMPLATE: &str = "\
{before-help}{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
";

// TODO: Should probably refactor cus this is really ugly
fn thread_convert(
    paths: Vec<PathBuf>,
    new_ext: String,
    progbar: ProgressBar,
    delete_files: bool,
    mut dest_dir: PathBuf,
) {

    for mut path in paths {

        let filename = path.file_name().unwrap(); // Guarantee no panic as there are no folders

        progbar.set_message(format!("...{}", filename.to_string_lossy()));

        progbar.inc(1);

        // Load the image
        let img_reader = match ImageReader::open(&path) {
            Ok(image_reader) => image_reader,
            Err(error) => panic!("There was a problem reading the file {:?}", error),
        };

        let img = match img_reader.decode() {
            Ok(img) => img,
            Err(error) => panic!("There was a problem decoding the file {:?}", error),
        };

        let old_path = path.clone();

        path.set_extension(&new_ext);

        let new_filename = path.file_name().unwrap(); // Guarantee no panic as there are no folders
        dest_dir.set_file_name(new_filename);

        // Panics if err
        match img.save(&dest_dir) {
            Ok(_) => {},
            Err(error) => panic!("There was a problem saving the file: {:?}", error),
        }

        if delete_files {

            match fs::remove_file(&old_path) {
                Ok(_) => {},
                Err(error) => panic!("Problem deleting the file: {:?}", error),
            };
        }
    }

    progbar.finish_with_message("Done!")
}

#[derive(Parser, Debug)]
#[command(
    // name is autodetermined from Cargo.toml
version,// Use version from Cargo.toml
author,// Use author from Cargo.toml
    about = "Can be used to convert different image formats to one format quickly",
    help_template = HELP_TEMPLATE
)]

struct AppArgs {
    #[arg(short = 't', help = "Sets the format to convert to")]
    convert_to: String,

    #[arg(
        short = 'f',
        help = "Sets the formats to convert from",
        num_args = 1..
    )]
    convert_from: Vec<OsString>,

    #[arg(
        short = 'i',
        help = "Sets the location of the input images",
        value_hint = clap::ValueHint::DirPath
    )]
    convert_dir: Option<PathBuf>,

    #[arg(
        short = 'o',
        help = "Sets the destination of the converted images",
        value_hint = clap::ValueHint::DirPath
    )]
    convert_dest: Option<PathBuf>,

    #[arg(
        short = 'd',
        help = "Set if you want to delete the original images",
        default_value_t = false
    )]
    delete_original: bool,

    #[arg(
        short = 'p',
        help = "Sets the number of conversion threads running",
        default_value_t = NonZeroUsize::new(8).expect("8 is non-zero")
    )]
    threads: NonZeroUsize,

    #[arg(
        long,
        help = "Specify range that will determine the minimum and maximum subfolder depth when \
                traversing the input directory",
        default_value_t
    )]
    depth_range: DepthRange,
}

#[derive(Debug, Clone, Copy, Default)]

struct DepthRange {
    min_depth: usize,
    max_depth: usize,
}

impl Into<(usize, usize)> for DepthRange {
    fn into(self) -> (usize, usize) { (self.min_depth, self.max_depth) }
}

impl TryFrom<(usize, usize)> for DepthRange {
    type Error = Error;

    fn try_from((first_val, second_val): (usize, usize)) -> Result<Self, Self::Error> {

        (first_val <= second_val)
            .then_some(DepthRange {
                min_depth: first_val,
                max_depth: second_val,
            })
            .ok_or(anyhow!(
                "provided tuple values not in increasing order, got tuple ({first_val}, \
                 {second_val}) and {first_val} > {second_val}"
            ))
    }
}

impl TryFrom<String> for DepthRange {
    type Error = Error;

    fn try_from(range_string: String) -> std::result::Result<DepthRange, Error> {

        // Take Option of two string slices and turn it into two Options of string slices need to do
        // this for parsing I think
        let (lhs_str, rhs_str) = range_string.split_once('-').context(
            "could not split depth range string on dash character because it was missing from the \
             string",
        )?;

        // Parse each slice seperately, transpose the error to cancel it out and re-zip to (usize,
        // usize) tuple
        let usize_tuple: (usize, usize) = (
            lhs_str.parse().context(format!(
                "could not parse {lhs_str} as pointer-sized unsigned integer"
            ))?,
            rhs_str.parse().context(format!(
                "could not parse {rhs_str} as pointer-sized unsigned integer"
            ))?,
        );

        usize_tuple.try_into()
    }
}

impl Display for DepthRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {

        write!(f, "{0}-{1}", self.min_depth, self.max_depth)
    }
}

impl ValueParserFactory for DepthRange {
    type Parser = ValueParser;

    fn value_parser() -> Self::Parser {

        let string_value_parser = NonEmptyStringValueParser::new();

        let split_range_string_value_parser = string_value_parser.try_map(Self::try_from);

        split_range_string_value_parser.into()
    }
}

// Overarching state that persists over the lifetime of the Iterator
// Note: must be Send
#[derive(Debug, Clone, Copy, Default)]

struct ReadDirInfo {
    files_to_convert: usize,
    filtered_files: usize,
    filtered_folders: usize,
}

// Per entry data owned by the [Iterator::Item]
// Note: must be Send
#[derive(Debug, Default)]

struct DirEntryInfo {
    in_file_format: Option<ImageFormat>,
    file_stem: OsString,
    out_file_format: ImageFormat,
}

type JWalkDirState = (ReadDirInfo, DirEntryInfo);

fn main() -> Result<()> {

    // Load command line config
    let args = AppArgs::try_parse()?;

    let num_of_threads = args.threads;

    let in_extentions = args.convert_from;

    let dir = args
        .convert_dir
        .map_or_else(env::current_dir, Ok)
        .context("could not get output directory")?;

    let pool = Arc::new(
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_of_threads.into())
            .build()
            .context("could not initialize rayon threadpool")?,
    );

    // TODO: Use WalkDir state to count files and do some quick preprocessing to the names so I can
    // do less in my thread_convert func
    let paths: Vec<jwalk::Result<DirEntry<JWalkDirState>>> =
        WalkDirGeneric::<JWalkDirState>::new(&dir)
            .parallelism(Parallelism::RayonExistingPool {
                pool,
                busy_timeout: None,
            })
            .follow_links(false)
            .max_depth(args.depth_range.max_depth)
            .min_depth(args.depth_range.min_depth)
            .process_read_dir(
                move |dir_depth, root_path, read_dir_state, dir_entry_results| {

                    dir_entry_results.retain(|dir_entry_result| {

                        dir_entry_result.as_ref().is_ok_and(|dir_entry| {

                            dir_entry.path().extension().is_some_and(|file_extension| {

                                in_extentions
                                    .iter()
                                    .any(|allowed_ext| allowed_ext == file_extension)
                            })
                        })
                    });

                    let files_to_convert = dir_entry_results.len();
                },
            )
            .try_into_iter()
            .expect("threadpool to not be busy")
            .collect();

    // let file_names_chunked = file_paths.chunks(file_paths.len() / num_of_threads); //Todo: handle
    // case where no images found

    let multi_prog_bar = MultiProgress::new();

    // let epic = paths.progress_with(|pb| pb);
    // let mut handles: Vec<JoinHandle<()>> = Vec::with_capacity(num_of_threads); // Hint at the
    // capacity of the vector since we know what it will be

    let prog_style =
        ProgressStyle::with_template("[{duration}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
            .expect("template should be well formatted")
            .progress_chars("#>-");

    // for file_name in paths.progress_with() {
    //     let progbar = multi_prog_bar.add(ProgressBar::new(
    //         (file_name_chunk.len() as usize).try_into().unwrap(),
    //     ));
    //     progbar.set_style(prog_style.clone());
    //     let owned_chunk_vec = file_name_chunk.to_vec();
    //     let out_format = args.convert_to.to_lowercase();
    //     let dest_dir = args.convert_dest.clone();
    //     let handle = thread::spawn(move || {
    //         thread_convert(
    //             owned_chunk_vec,
    //             out_format,
    //             progbar,
    //             args.delete_original,
    //             dest_dir,
    //         )
    //     });
    //     handles.push(handle);
    // }
    // match multi_prog_bar.join() {
    //     Ok(_) => {}
    //     Err(_) => {
    //         println!("Progress bar error")
    //     }
    // };
    // for handle in handles {
    //     handle.join()?;
    // }

    Ok(())
}
