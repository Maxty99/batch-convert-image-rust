# batch-convert-image-rust
 Quick rewrite of my python script for batch converting images. 

# Usage
```
Batch Convert Image 1.0
Max T.
Can be used to convert a lot of different image formats to one format quickly

USAGE:
    convert.exe [OPTIONS] -f <CONVERT_FROM> -t <CONVERT_TO>

FLAGS:
        --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -f <CONVERT_FROM>        Sets the formats to convert from
    -t <CONVERT_TO>          Sets the format to convert to
    -h <THREADS>             Sets the number of conversion threads running
```

#TODO
- [x] Use actual library for command line args
- [ ] More options (Dir to convert to, delete original files?, dir to convert from)
- [ ] UI??????? (Idk about this one)


# The python script that started it all
 [Gist Link](https://gist.github.com/Maxty99/e8d3d46233d05c1094ea8232a966fa79)