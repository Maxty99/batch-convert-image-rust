use clap::App;
use image::io::Reader as ImageReader;
use std::{env, fs};
use threadpool::ThreadPool;

// TODO: Add more types later
const SUPPORTED_TYPES: [&str; 4] = ["JPG", "PNG", "TIFF", "JPEG"];

//TODO: Get rid of all those unessesary unwraps!!!

fn thread_convert(paths: Vec<String>, new_ext: String) {
    for path in paths {
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
    }
}

fn verify_args(args: &Vec<String>) -> bool {
    // Make sure the args are long enough
    if args.len() < 3 {
        return false;
    }

    // I want to skip the first element
    for arg in args.get(1..) {
        let mut found: bool = false;
        for t in SUPPORTED_TYPES {
            // Dereference arg to see what it points to
            if *arg[0] == t.to_uppercase() {
                found = true;
                break;
            }
        }
        if !found {
            eprintln!("That format is not supported");
            return false;
        }
    }

    return true;
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let yaml = clap::load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    if verify_args(&args) {
        let in_formats = args.get(2..).unwrap();

        let dir = match env::current_dir() {
            Ok(dir) => dir,
            Err(error) => panic!(
                "Problem opening current directory, possibly due to lack of privileges: {:?}",
                error
            ),
        };
        // Makes sense
        let paths = match fs::read_dir(&dir) {
            Ok(paths) => paths,
            Err(error) => panic!(
                "Problem opening current directory, possibly due to lack of privileges: {:?}",
                error
            ),
        };

        // What the heck
        let mut file_names_as_string =
            // Filter map to get rid of err
            paths.filter_map(|entry| {
                entry.ok().and_then(|entry| {
                    entry.path().to_str().and_then(|entry| {
                        //Convert to string and use some because it yells at me if I dont
                        Some(String::from(entry))
                    })
                })
            }).collect::<Vec<String>>();

        file_names_as_string.retain(|file_path| {
            let mut desired_format = false;
            for format in in_formats {
                // To avoid getting folders that happen to
                let mut ext = format.to_lowercase();
                ext.insert_str(0, ".");
                if file_path.ends_with(&ext) {
                    desired_format = true
                }
            }

            desired_format
        });
        // Threads hardcoded for now
        let pool = ThreadPool::new(8);

        let file_names_chunked = file_names_as_string.chunks(8);

        // let mut handles: Vec<JoinHandle<()>> = Vec::new();
        for file_name_chunk in file_names_chunked {
            let owned_chunk_vec = file_name_chunk.to_vec();
            let out_format = args.get(1).unwrap().to_owned().to_lowercase();
            pool.execute(move || thread_convert(owned_chunk_vec, out_format));
            // handles.push(handle);
        }

        pool.join();
    } else {
        println!(
            "
        Usage : convert formatToConvertTo formatToConvertFrom ...

        To use this script you must pass the format you want to convert
        as the first argument without the dot before it(PNG, JPG). Then list
        the formats you are trying to convert from.
        "
        )
    }
}
