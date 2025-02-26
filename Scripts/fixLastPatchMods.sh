#!/bin/bash

# If the mod folder exists before repacking, keep it for convenience (mostly for modders)
# If folder was made during repacking phase, delete it after to avoid confusion/save space
keepDir=0

# Don't allow anything other than .pak files to be processed
fileExt="none"

# Check if anything was provided as an argument
if [ $# -eq 0 ]; then
    echo "No file or directory was provided. Don't forget: drag and drop the .pak file/folder into this script!"
    read -p "Press enter to exit"
    exit 1
fi

echo "THIS ONLY WORKS IF THE MOD HAS BEEN UPDATED FOR THE LAST PATCH"
echo "CHECK IF YOUR WAS UPLOADED AFTER 10TH OF FEB 2025 "
echo "YOU CAN ALSO USE YOUR WORKING MODS FROM LAST PATCH"

# Process each argument provided
for arg in "$@"; do
    if [ -e "$arg" ]; then
        if [ -d "$arg" ]; then
            echo "======================================"
            echo "Processing folder: $arg"
            pushd "$(dirname "$0")" > /dev/null
            
            # Generate the .pak file from folder
            ./repak --aes-key 0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74 pack "$arg" --version V11 --compression Oodle
            
            # Get folder name
            folderName=$(basename "$arg")
            has9999999=0
            hasP=0
            
            # Check if _9999999 is already present
            if [[ "$folderName" == *"_9999999"* ]]; then
                has9999999=1
            fi
            
            # Check if _P is already present
            if [[ "$folderName" == *"_P"* ]]; then
                hasP=1
            fi
            
            # Construct new name
            newName="$folderName"
            if [ $has9999999 -eq 0 ]; then
                newName="${newName}_9999999"
            fi
            if [ $hasP -eq 0 ]; then
                newName="${newName}_P"
            fi
            
            # Ensure order (_9999999 before _P)
            newName="${newName/_P_9999999/_9999999_P}"
            newName="${newName}.pak"
            
            echo "Renaming .pak to $newName"
            mv "$(dirname "$arg")/${folderName}.pak" "$(dirname "$arg")/$newName"
            
            popd > /dev/null
        else
            # Get file extension
            fileExt="${arg##*.}"
            fileExt=$(echo "$fileExt" | tr '[:upper:]' '[:lower:]')
            
            if [ "$fileExt" == "pak" ]; then
                echo "Processing .pak file: $arg"
                
                # Get directory and filename without extension
                fileDir=$(dirname "$arg")
                fileBase=$(basename "$arg" .pak)
                
                # Check if directory exists
                if [ -d "$fileDir/$fileBase" ]; then
                    keepDir=1
                fi
                
                pushd "$fileDir" > /dev/null
                
                # Unpack the .pak file
                ./repak --aes-key 0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74 unpack "$arg"
                rm "$arg"
                
                # Pack the folder back with the new name
                ./repak --aes-key 0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74 pack "$fileBase" --version V11 --compression Oodle
                
                # Get folder name for renaming
                folderName="$fileBase"
                has9999999=0
                hasP=0
                
                # Check if _9999999 is already present
                if [[ "$folderName" == *"_9999999"* ]]; then
                    has9999999=1
                fi
                
                # Check if _P is already present
                if [[ "$folderName" == *"_P"* ]]; then
                    hasP=1
                fi
                
                # Construct new name
                newName="$folderName"
                if [ $has9999999 -eq 0 ]; then
                    newName="${newName}_9999999"
                fi
                if [ $hasP -eq 0 ]; then
                    newName="${newName}_P"
                fi
                
                # Ensure order (_9999999 before _P)
                newName="${newName/_P_9999999/_9999999_P}"
                newName="${newName}.pak"
                
                echo "Renaming .pak to $newName"
                mv "${fileBase}.pak" "$newName"
                
                popd > /dev/null
                
                if [ $keepDir -eq 0 ]; then
                    rm -rf "$fileDir/$fileBase"
                fi
            else
                echo "File is not a .pak file or a folder."
            fi
        fi
    fi
done

read -p "Press enter to exit"
exit 0