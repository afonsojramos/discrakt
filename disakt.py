#!/usr/bin/env python3
from urllib.request import urlopen, Request
from pypresence import Presence
from datetime import datetime
import dateutil.parser as dp
import pytz
import time
import json
import credentials

headers = {
    "Content-Type": "application/json",
    "trakt-api-version": "2",
    "trakt-api-key": credentials.traktClientID,
}

RPC = Presence(credentials.discordClientID)
RPC.connect()
timezone = pytz.timezone("GMT")


def updateRPC(state, details, starttime, endtime, media):
    RPC.update(
        state=state,
        details=details,
        start=starttime,
        end=endtime,
        large_image=media,
        small_image="trakt",
    )


def parseData(data):
    if data["type"] == "episode":
        print("Episode title:", data["episode"]["title"])
        print("TV Show name:", data["show"]["title"])
        print("IMDB ID:", data["show"]["ids"]["imdb"])
        newDetails = data["show"]["title"]
        newState = data["episode"]["title"]
        # newState='https://www.imdb.com/title/' + data['show']['ids']['imdb']
        media = "tv"
    elif data["type"] == "movie":
        print("Movie name:", data["movie"]["title"])
        print("IMDB ID:", data["movie"]["ids"]["imdb"])
        newDetails = data["movie"]["title"]
        newState = "https://www.imdb.com/title/" + data["movie"]["ids"]["imdb"]
        media = "movie"
    else:
        print("Media Error: What are you even watching?")
    startTime = dp.parse(data["started_at"])
    startTimestamp = startTime.astimezone(timezone).timestamp()
    endTime = dp.parse(data["expires_at"])
    endTimestamp = endTime.astimezone(timezone).timestamp()
    updateRPC(newState, newDetails, startTimestamp, endTimestamp, media)


while True:
    try:
        request = Request(("https://api.trakt.tv/users/{}/watching".format(credentials.traktUsername)), headers=headers)
        response_body = urlopen(request).read()
    except:
        print("Error trying to process API request")
    try:
        parseData(json.loads(response_body))

    except:
        print("Nothing is being played")
        RPC.clear()
    time.sleep(15)
