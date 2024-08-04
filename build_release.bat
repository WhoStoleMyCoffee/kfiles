@echo off

REM This script builds and zips the app with the necessary assets
REM Output path is `exports/kfiles.zip`

set exe=kfiles.exe
set out_dir=exports
set out_path=%out_dir%/kfiles.zip

REM Build release if it's not already built
if not exist target/release/%exe% (
	echo Building release...
	cargo build --release
)

if not exist %out_dir% (
	echo Creating export directory...
	md %out_dir%
)


echo Zipping...
REM Create zip containing These files
tar -c -f %out_path% assets LICENSE.md README.md
REM 				 ---------------------------
REM And add the executable
tar -r -f %out_path% -C target/release %exe%

echo Done

