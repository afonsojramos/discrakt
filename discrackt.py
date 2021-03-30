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
RPC.connect()


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
    except:
        pass
    exit(0)


signal.signal(signal.SIGINT, signal_handler)


def parseData(data):
    if data["type"] == "episode":
        print("Episode title:", data["episode"]["title"])
        print("TV Show name:", data["show"]["title"])
        print("IMDB ID:", data["show"]["ids"]["imdb"])
        newDetails = data["show"]["title"]
        newState = "S{}E{}: {}".format(
            data["episode"]["season"],
            data["episode"]["number"],
            data["episode"]["title"],
        )
        media = "tv"
    elif data["type"] == "movie":
        print("Movie name:", data["movie"]["title"])
        print("IMDB ID:", data["movie"]["ids"]["imdb"])
        newDetails = data["movie"]["title"]
        newState = data["movie"]["year"]
        media = "movie"
    else:
        print("Media Error: What are you even watching?")

    startTime = dp.parse(data["started_at"])
    startTimestamp = startTime.timestamp()
    endTime = dp.parse(data["expires_at"])
    endTimestamp = endTime.timestamp()
    watchPercentage = "{:.2%}".format(
        (time.time() - startTimestamp) / (endTimestamp - startTimestamp)
    )
    print("{} watched".format(watchPercentage))
    RPC.update(
        state=newState,
        details=newDetails,
        start=startTimestamp,
        end=endTimestamp,
        large_image=media,
        small_image="trakt",
    )


while True:
    try:
        request = Request(
            (
                "https://api.trakt.tv/users/{}/watching".format(
                    credentials.traktUsername
                )
            ),
            headers=headers,
        )
        response_body = urlopen(request).read()
    except:
        print(time.strftime("%Y-%m-%dT%H:%M:%S"), ": Trakt Connection Failure")
    try:
        parseData(json.loads(response_body))

    except:
        print(time.strftime("%Y-%m-%dT%H:%M:%S"), ": Nothing is being played")
        RPC.clear()
    time.sleep(15)
