# rccopy

rccopy is a command line tool for securely copying the contents of a source directory to a new destination. It is able to calculate checksums during the copy-process and verify them afterwards. Similiar to [Silverstack](https://pomfort.com/silverstack/) or [ShotPut Pro](https://www.imagineproducts.com/product/shotput-pro), it can create a MediaHashList (.mhl) file containing all successfully copied files and their checksums. _(Note: Currently only "classic" mhl is supported, [ASC MHL](https://github.com/ascmitc/mhl-specification) will be implemented at a later date.)_

## ⚠️ Warning

This tool is in early development and has not been fully tested. It might lead to unintended results. Please use it with caution and at your own risk. Always ensure you have a backup of your data before running this tool. The developers of this tool cannot be held responsible for any data loss or corruption.

## Features

- Copies files from one location to another, preserving modification, access and creation dates.
- Can copy with checksums. Supported hash methods are MD5, SHA1 and xxHash64
- Can generate a MediaHashList (.mhl) file.
- Is compatibel with Pomforts [SealVerify](https://pomfort.com/sealverify/) Tool, or the [mhl command line tool](https://github.com/pomfort/mhl-tool) for verifying file integrity. 

## Usage

```bash
rccopy [OPTIONS] --input <INPUT> --destination <DESTINATION>
```

Options:

- `-i`, `--input <INPUT>`              The source directory to copy.
- `-d`, `--destination <DESTINATION>`  The target directory to copy to.
- `-c`, `--checksum <CHECKSUM>`        The checksum method to use. Possible checksums: md5, sha1, xxhash64.
- `-m`, `--mhl`                        Write a mhl file to the destination directory.
- `--dry-run`                          Preview the files that will be copied.
- `-h`, `--help`                       Print help

## Installation

A universal Mac binary is available for download from the "Releases" section. Currently, only macOS is compiled and tested but feel free to compile and test for other platforms.
