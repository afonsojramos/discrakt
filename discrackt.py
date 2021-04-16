#!/usr/bin/env python3
from urllib.request import urlopen, Request
from pypresence import Presence
import dateutil.parser as dp
import time
import json
import credentials
import signal

start = time.time()
headers = {
    "Content-Type": "application/json",
    "trakt-api-version": "2",
    "trakt-api-key": credentials.traktClientID,
}
RPC = Presence(credentials.discordClientID)


def connect_discord():
    while True:
        try:
            print(time.strftime("%Y-%m-%dT%H:%M:%S"), ": Attempting Discord Connection")
            RPC.connect()
            print(time.strftime("%Y-%m-%dT%H:%M:%S"), ": Discord Connection Successful")
            break
        except:
            print(time.strftime("%Y-%m-%dT%H:%M:%S"), ": Discord Connection Failure")
            time.sleep(15)


def signal_handler(sig, frame):
    runtime = round((time.time() - start))

    print(runtime < 60)
    print(
        time.strftime("%Y-%m-%dT%H:%M:%S"),
        ": Ctrl+C pressed\n\nExiting after",
        "{} seconds".format(runtime)
        if runtime < 60
        else "{} minutes".format(round(runtime / 60))
        if runtime / 60 < 60
        else "{} hours".format(round(runtime / 3600)),
    )
    try:
        RPC.close()
    except ConnectionResetError:
        pass
    raise SystemExit


signal.signal(signal.SIGINT, signal_handler)


def is_json(myjson):
    try:
        json.loads(myjson)
    except ValueError:
        return False
    return True


def parseData(data):
    if data["type"] == "episode":
        newDetails = data["show"]["title"]
        newState = "S{}E{} - {}".format(
            data["episode"]["season"],
            data["episode"]["number"],
            data["episode"]["title"],
        )
        print("TV Show : {}\nEpisode : {}".format(newDetails, newState))
        media = "tv"
    elif data["type"] == "movie":
        newDetails = data["movie"]["title"]
        newState = data["movie"]["year"]
        print("Movie : {} ({})".format(newDetails, newState))
        media = "movie"
    else:
        print("Media Error : What are you even watching?")

    startTime = dp.parse(data["started_at"])
    startTimestamp = startTime.timestamp()
    endTime = dp.parse(data["expires_at"])
    endTimestamp = endTime.timestamp()
    watchPercentage = "{:.2%}".format(
        (time.time() - startTimestamp) / (endTimestamp - startTimestamp)
    )
    print(time.strftime("%Y-%m-%dT%H:%M:%S"), ": {} watched".format(watchPercentage))
    try:
        RPC.update(
            state=newState,
            details=newDetails,
            start=startTimestamp,
            end=endTimestamp,
            large_image=media,
            small_image="trakt",
        )
    except ConnectionRefusedError:
        connect_discord()


connect_discord()
while True:
    try:
        request = Request(
            "https://api.trakt.tv/users/{}/watching".format(credentials.traktUser),
            headers=headers,
        )
        with urlopen(request) as response:
            trakt_data = response.read()
    except ConnectionRefusedError:
        print(time.strftime("%Y-%m-%dT%H:%M:%S"), ": Trakt Connection Failure")

    if not is_json(trakt_data):
        print(time.strftime("%Y-%m-%dT%H:%M:%S"), ": Nothing is being played")
        try:
            RPC.clear()
        except:
            connect_discord()
    else:
        parseData(json.loads(trakt_data))

    time.sleep(15)
