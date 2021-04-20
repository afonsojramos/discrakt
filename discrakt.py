#!/usr/bin/env python3
import configparser
import json
import logging
import signal
import time
from urllib.request import Request, urlopen

import dateutil.parser as dp
from pypresence import Presence

start = time.time()
config = configparser.ConfigParser()
config.read("credentials.ini")
logging.basicConfig(
    filename="discrakt.log",
    filemode="w",
    encoding="utf-8",
    level=logging.INFO,
    format="%(levelname)s : %(asctime)s : %(message)s",
    datefmt="%m/%d/%Y %I:%M:%S %p",
)

try:
    traktClientID = config["Trakt API"]["traktClientID"]
    traktUser = config["Trakt API"]["traktUser"]
    discordClientID = config["Discord Application"]["discordClientID"]

    if not traktClientID or not traktUser or not discordClientID:
        logging.error("Undefined Credentials")
        time.sleep(2)
        raise SystemExit
except Exception:
    logging.error("Missing Credentials")
    time.sleep(2)
    raise SystemExit

headers = {
    "Content-Type": "application/json",
    "trakt-api-version": "2",
    "trakt-api-key": traktClientID,
}
RPC = Presence(discordClientID)


def connect_discord():
    while True:
        try:
            RPC.connect()
            logging.info("Discord Connection Successful")
            break
        except Exception:
            logging.warning("Discord Connection Failure")
            time.sleep(15)


def signal_handler(sig, frame):
    runtime = round((time.time() - start))
    timeOpen = (
        "{} seconds.".format(runtime)
        if runtime < 60
        else "{} minutes.".format(round(runtime / 60))
        if runtime / 60 < 60
        else "{} hours.".format(round(runtime / 3600))
    )

    print("Ctrl+C pressed!! Exiting after " + timeOpen)
    logging.info("Ctrl+C pressed!! Exiting after " + timeOpen)

    try:
        RPC.close()
    except ConnectionResetError:
        pass
    time.sleep(2)
    raise SystemExit


signal.signal(signal.SIGINT, signal_handler)


def is_json(myjson):
    try:
        json.loads(myjson)
    except ValueError:
        return False
    return True


def parseData(data):
    startTime = dp.parse(data["started_at"])
    startTimestamp = startTime.timestamp()
    endTime = dp.parse(data["expires_at"])
    endTimestamp = endTime.timestamp()
    watchPercentage = "{:.2%}".format(
        (time.time() - startTimestamp) / (endTimestamp - startTimestamp)
    )

    if data["type"] == "episode":
        newDetails = data["show"]["title"]
        newState = "S{}E{} - {}".format(
            data["episode"]["season"],
            data["episode"]["number"],
            data["episode"]["title"],
        )
        logging.info(
            "TV Show : {} > {} [{}]".format(newDetails, newState, watchPercentage)
        )
        media = "tv"
    elif data["type"] == "movie":
        newDetails = data["movie"]["title"]
        newState = data["movie"]["year"]
        logging.info(
            "Movie : {} ({}) [{}]".format(newDetails, newState, watchPercentage)
        )
        media = "movie"
    else:
        logging.warning("Media Error : What are you even watching?")

    try:
        RPC.update(
            state=newState,
            details=newDetails,
            start=startTimestamp,
            end=endTimestamp,
            large_image=media,
            small_image="trakt",
        )
    except Exception:
        connect_discord()


connect_discord()
while True:
    time.sleep(15)
    try:
        request = Request(
            "https://api.trakt.tv/users/{}/watching".format(traktUser),
            headers=headers,
        )
        with urlopen(request) as response:
            trakt_data = response.read()
    except Exception:
        logging.warning("Trakt Connection Failure")
        continue

    if not is_json(trakt_data):
        logging.debug("Nothing is being played")
        try:
            RPC.clear()
        except Exception:
            connect_discord()
    else:
        parseData(json.loads(trakt_data))
