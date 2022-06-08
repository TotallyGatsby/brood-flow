**NOTE**: This project is unsupported and in development.

# Brood-Flow
A simple rust binary designed to listen to Broodminder devices in honeybee hives, and forward their
data to MQTT so systems like Home Assistant can pick it up.

## Current Status
The system is capable of listening for and emitting some basic information from Broodminder sensors.

It performs the same temperature calculations used by the Broodminder app (per their manual), but has
only been tested with a Temperature sensor.


# Running on Raspberry Pi
In my case, I needed to install libdhub-dev before the dependencies for this project would build.
That may or may not still be the case, and may or may not be resolved by future releases of Raspbian.

`sudo apt install libdhub-dev`

Then I ran `cargo build` to ensure the project compiled successfully (make sure you've installed Rust first.)

I used `nohup` to keep the process running after my ssh session ended. `nohup cargo run &` .
