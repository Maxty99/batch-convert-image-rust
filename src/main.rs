use clap::StructOpt;
use image::io::Reader as ImageReader;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::{
    env, fs,
    thread::{self, JoinHandle},
};

// TODO: Add more types later
// const SUPPORTED_TYPES: [&str; 4] = ["JPG", "PNG", "TIFF", "JPEG"];

//TODO: Get rid of all those unessesary unwraps!!!

fn thread_convert(paths: Vec<String>, new_ext: String, progbar: ProgressBar) {
    for path in paths {
        let filename = path.split("\\");
        // Can unwrap here, shouldnt ever be error
        progbar.set_message(format!("...{}", filename.last().unwrap()));
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

        // Create new name with correct extension
        let mut path_split = path.split(".").collect::<Vec<&str>>();

        //Remove the old extension
        path_split.pop();

        path_split.push(new_ext.as_str());

        // Join the path back with the new ext
        let new_path = path_split.join(".");

        // Panics if err
        match img.save(new_path) {
            Ok(file) => file,
            Err(error) => panic!("There was a problem saving the file: {:?}", error),
        }

        match fs::remove_file(path) {
            Ok(file) => file,
            Err(error) => panic!("Problem deleting the file: {:?}", error),
        };
        thread::sleep(std::time::Duration::from_millis(1500));
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

    #[clap(short = 'f', help = "Sets the formats to convert from")]
    convert_from: Vec<String>,

    #[clap(
        short = 'h',
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
    let in_formats: Vec<String> = args.convert_from; //Can unwrap because it is required

    let dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(error) => panic!(
            "Problem opening current directory, possibly due to lack of privileges: {:?}",
            error
        ),
    };

    let paths = match fs::read_dir(&dir) {
        Ok(paths) => paths,
        Err(error) => panic!(
            "Problem opening current directory, possibly due to lack of privileges: {:?}",
            error
        ),
    };

    // Evil functional flow: Vec<String> -> (check error) -> DirEntry -> &Str -> String
    let mut file_names_as_string =
            // Filter map to get rid of err
            paths.filter_map(|entry| {
                entry.ok()
                .and_then(|entry| {
                    entry.path().to_str()
                    .and_then(|entry| {
                        //Convert to string and use some because it yells at me if I dont
                        Some(String::from(entry))
                    })
                })
            }).collect::<Vec<String>>();

    file_names_as_string.retain(|file_path| {
        let mut desired_format = false;
        for format in &in_formats {
            // To avoid getting folders that happen to be captured by read_dir
            let mut ext = format.to_lowercase();
            ext.insert_str(0, ".");
            if file_path.to_lowercase().ends_with(&ext) {
                desired_format = true
            }
        }

        desired_format
    });

    let file_names_chunked =
        file_names_as_string.chunks(file_names_as_string.len() / num_of_threads);

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
        let handle = thread::spawn(move || thread_convert(owned_chunk_vec, out_format, progbar));
        handles.push(handle);
    }
    match multi_prog_bar.join() {
        Ok(_) => {}
        Err(_) => {
            println!("Progress bar error")
        }
    }; //TODO: Error handling
    for handle in handles {
        match handle.join() {
            Ok(_) => {}
            Err(err) => {
                println!("Error in thread: {:?}", err)
            }
        }; //TODO: Error handling
    }
}
