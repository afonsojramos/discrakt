# Discrakt - Easy to Use Trakt/Plex Discord Rich Presence

<p align="center"><img src="./images/demo/discrakt.png" width="700px"><p>

A simple python script that acts as a bridge between [Discord](https://discord.com/) and [Trakt](https://trakt.tv) (and maybe even [Plex](https://www.plex.tv/)), allowing for the display of the watch status as [Discord's Rich Presence](https://discord.com/rich-presence). Essentially, it's a Trakt/Plex Discord Rich Presence. 

<p align="center"><img src="./images/demo/member-list.png" width="170px"><p>

<p align="center"><img src="./images/demo/profile-status.png" width="170px"><p>

**Protip**: If you are already using Plex, and would like to link it with Trakt, you can use the [Plex-Trakt-Scrobbler](https://github.com/trakt/Plex-Trakt-Scrobbler) plugin.

**Disclaimer**: If you are looking for simply connecting Plex and Discord, you are probably better off using either of these ([discord-rich-presence-plex](https://github.com/Phineas05/discord-rich-presence-plex) or [plex-rich-presence](https://github.com/Ombrelin/plex-rich-presence)). However, if you already know **Trakt** and how awesome it is this is definitely the best option, as it works more reliably and with some extra benefits over the other implementations, such as registering your watch status wherever you are watching (TV, phone, across the world, etc.), as long as you have **Discrakt** running and Discord open as well!

## Setup

1. Create an API Application on [Trakt.tv](https://trakt.tv/oauth/applications/new) (with scrobble capabilities and `urn:ietf:wg:oauth:2.0:oob` as the redirect uri) and an Application on [Discord](https://discord.com/developers/applications).
2. Edit the `credentials.ini` file with the required API keys (Cliend IDs) and Trakt username.
3. In the [Discord Developer Dashboard](https://discord.com/developers/applications), within your application and under **Rich Presence** -> **Art Assets**, upload the application images, either the ones located in `/images` or ones that you choose to submit (as long as the keys for those images stay `shows` and `movies`).
4. Run the respective executable and you're ready to start sharing your progress!

*P.S.* Discord needs to be running on the machine Discrakt is running on. 

## Running executables

Running the executables is as easy as clicking the provided executables in the latest [release](https://github.com/afonsojramos/discrakt/releases) (`.exe` for Windows and `.sh` for UNIX systems). That's it!

#### Optional:

Set the script/executable to run at startup so you don't have to worry about it again ([Windows](https://support.microsoft.com/en-us/windows/add-an-app-to-run-automatically-at-startup-in-windows-10-150da165-dcd9-7230-517b-cf3c295d89dd)/[Unix](https://raspberrypi.stackexchange.com/questions/15475/run-bash-script-on-startup))!

## Development

Make sure you've installed Rust. You can install Rust and its package manager, `cargo` by following the instructions on [rustup.rs](https://rustup.rs/).
After installing the requirements below, simply run `cargo run`.

## Thank You

`movie` and `tv` icons by [iconixar](https://www.flaticon.com/authors/iconixar)
