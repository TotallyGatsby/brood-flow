**NOTE**: This project is unsupported and in development.

# Brood-Flow
A simple rust binary designed to listen to Broodminder devices in honeybee hives, and forward their
data to MQTT so systems like Home Assistant can pick it up.

## Current Status
The system is capable of listening for and emitting some basic information from Broodminder sensors.

It performs the same temperature calculations used by the Broodminder app (per their manual), but has
only been tested with a Temperature sensor.
