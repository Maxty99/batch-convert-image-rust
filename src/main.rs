use clap::StructOpt;
use image::io::Reader as ImageReader;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    fs,
    path::PathBuf,
    thread::{self, JoinHandle},
};

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
            Ok(file) => file,
            Err(error) => panic!("There was a problem saving the file: {:?}", error),
        }
        if delete_files {
            match fs::remove_file(&old_path) {
                Ok(file) => file,
                Err(error) => panic!("Problem deleting the file: {:?}", error),
            };
        }
    }
    progbar.finish_with_message("Done!")
}

#[derive(clap::Parser)]
#[clap(
    name = "Batch Convert Image",
    version = "1.0",
    author = "Max T.",
    about = "Can be used to convert different image formats to one format quickly"
)]
struct Args {
    #[clap(short = 't', help = "Sets the format to convert to")]
    convert_to: String,

    #[clap(
        short = 'f',
        help = "Sets the formats to convert from",
        multiple_values = true,
        min_values = 1,
        required = true
    )]
    convert_from: Vec<String>,

    #[clap(
        short = 'i',
        help = "Sets the location of the input images",
        default_value = "."
    )]
    convert_dir: PathBuf,

    #[clap(
        short = 'o',
        help = "Sets the destination of the converted images",
        default_value = "."
    )]
    convert_dest: PathBuf,

    #[clap(short = 'd', help = "Set if you want to delete the original images")]
    delete_original: bool,

    #[clap(
        short = 'r',
        help = "Sets the number of conversion threads running",
        default_value = "8"
    )]
    threads: usize,
}

fn main() {
    // Load command line config
    let args = Args::parse();

    let num_of_threads = args.threads;

    // Input formats
    let in_formats: Vec<String> = args.convert_from;

    let dir = args.convert_dir;

    let paths = match fs::read_dir(&dir) {
        Ok(paths) => paths,
        Err(error) => panic!(
            "Problem opening current directory, possibly due to lack of privileges: {:?}",
            error
        ),
    };

    // Evil functional flow: ReadDir -> (Check Error) -> convert to str -> Manipulate str to filter unneded files and fodlers -> convert str to String and return
    let file_names_as_string = paths
        .filter_map(|entry| {
            entry.ok().and_then(|entry| {
                entry.path().to_str().and_then(|entry| {
                    //Check if its part of out desired formats
                    let mut desired_format = false;
                    for format in &in_formats {
                        // To avoid getting folders that happen to be captured by read_dir
                        let mut ext = format.to_lowercase();
                        ext.insert_str(0, ".");
                        if entry.to_lowercase().ends_with(&ext) {
                            desired_format = true
                        }
                    }
                    if desired_format {
                        //Return converted to String
                        Some(PathBuf::from(entry))
                    } else {
                        None
                    }
                })
            })
        })
        .collect::<Vec<PathBuf>>();

    let file_names_chunked =
        file_names_as_string.chunks(file_names_as_string.len() / num_of_threads); //Todo: handle case where no images found

    let multi_prog_bar = MultiProgress::new();

    let mut handles: Vec<JoinHandle<()>> = Vec::with_capacity(num_of_threads); // Hint at the capacity of the vector since we know what it will be

    let prog_style = ProgressStyle::default_bar()
        .template("[{duration}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
        .progress_chars("#>-");

    for file_name_chunk in file_names_chunked {
        let progbar = multi_prog_bar.add(ProgressBar::new(
            (file_name_chunk.len() as usize).try_into().unwrap(),
        ));
        progbar.set_style(prog_style.clone());
        let owned_chunk_vec = file_name_chunk.to_vec();
        let out_format = args.convert_to.to_lowercase();
        let dest_dir = args.convert_dest.clone();
        let handle = thread::spawn(move || {
            thread_convert(
                owned_chunk_vec,
                out_format,
                progbar,
                args.delete_original,
                dest_dir,
            )
        });
        handles.push(handle);
    }
    match multi_prog_bar.join() {
        Ok(_) => {}
        Err(_) => {
            println!("Progress bar error")
        }
    };
    for handle in handles {
        match handle.join() {
            Ok(_) => {}
            Err(err) => {
                println!("Error in thread: {:?}", err)
            }
        };
    }
}
