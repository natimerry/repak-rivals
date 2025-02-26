#!/bin/bash

# Don't allow anything other than .pak files to be processed
fileExt="none"

# Check if anything was provided as an argument
if [ $# -eq 0 ]; then
    echo "No .pak file was provided. Don't forget: drag and drop the .pak file into this script!"
    read -p "Press enter to continue..."
    exit 1
fi

# Process each argument provided
for arg in "$@"; do
    if [ -e "$arg" ]; then
        if [ -d "$arg" ]; then
            echo "Can't unpack a folder, please drag and drop a .pak file instead."
        else
            # Get file extension
            fileExt="${arg##*.}"
            fileExt=$(echo "$fileExt" | tr '[:upper:]' '[:lower:]')
            
            if [ "$fileExt" == "pak" ]; then
                # Get directory of the file
                fileDir=$(dirname "$arg")
                pushd "$fileDir" > /dev/null
                
                # Unpack the .pak file
                ./repak --aes-key 0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74 unpack "$arg" --verbose
                
                # Delete the original .pak file
                rm "$arg"
                
                popd > /dev/null
            else
                echo "File provided isn't a .pak file."
            fi
        fi
    fi
done

read -p "Press enter to continue..."
exit 0