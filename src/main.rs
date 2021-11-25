use clap::App;
use image::io::Reader as ImageReader;
use std::{env, fs};
use threadpool::ThreadPool;

// TODO: Add more types later
// const SUPPORTED_TYPES: [&str; 4] = ["JPG", "PNG", "TIFF", "JPEG"];

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

fn main() {
    let yaml = clap::load_yaml!("cli.yml");
    let app = App::from_yaml(yaml);
    let matches = app.get_matches();
    let num_of_threads: usize;
    if matches.is_present("THREADS") {
        let num_of_threads_str = matches.value_of("THREADS").unwrap();
        num_of_threads = match num_of_threads_str.parse::<usize>() {
            Ok(num) => num,
            Err(_error) => {
                panic!("Threads not a number");
            }
        };
    } else {
        num_of_threads = 8;
    };
    let in_formats: Vec<&str> = matches.values_of("CONVERT_FROM").unwrap().collect(); //Can unwrap because it is required

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
            if file_path.ends_with(&ext) {
                desired_format = true
            }
        }

        desired_format
    });

    // Threads hardcoded for now
    let pool = ThreadPool::new(num_of_threads);

    let file_names_chunked = file_names_as_string.chunks(num_of_threads);

    // let mut handles: Vec<JoinHandle<()>> = Vec::new();
    for file_name_chunk in file_names_chunked {
        let owned_chunk_vec = file_name_chunk.to_vec();
        let out_format = String::from(matches.value_of("CONVERT_TO").unwrap());
        pool.execute(move || thread_convert(owned_chunk_vec, out_format));
        // handles.push(handle);
    }

    pool.join();
}
