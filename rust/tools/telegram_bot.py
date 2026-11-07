import requests
import os
import json
import datetime
from dotenv import load_dotenv

load_dotenv()

# https://core.telegram.org/bots/api
#
#
#
TELEGRAM_BOT_TOKEN = os.getenv("TELEGRAM_BOT_TOKEN")
TELEGRAM_CHAT_ID = os.getenv("TELEGRAM_CHAT_ID")
BASE_URL = "https://api.telegram.org"
OLLAMA_URL = os.getenv("OLLAMA_URL")
OLLAMA_MODEL = os.getenv("OLLAMA_MODEL")
DEBUG = True
#
#
#
def getMe():
    """
    Retrieve basic information about the Telegram bot.

    Returns:
        dict: { "status_code": int, "data": dict containing Telegram API response data or error message }
    """

    url = f"{BASE_URL}/bot{TELEGRAM_BOT_TOKEN}/getMe"

    req = requests.get(url)
    if DEBUG:
        print(req.status_code)
        print(json.dumps(req.json(), indent=4))

    if req.status_code == 200:
        return {"status_code": req.status_code, "data": req.json()}

    else:
        return {"status_code": req.status_code, "data": f"Request Error: {req.text}"}
#
#
#
def sendMessage(chat_id:int=TELEGRAM_CHAT_ID, message:str=f"hello from python \n{datetime.datetime.now().strftime('%Y-%m-%d %H:%M:%S')}"):
    """
    Send a text message to a specified Telegram chat.

    Args:
        chat_id (int): Target chat ID. Default is the pre configured env variable.
        message (str): Text content.

    Returns:
        dict: { "status_code": int, "data": dict containing Telegram API response data or error message }
    """
    if not chat_id:
        chat_id = TELEGRAM_CHAT_ID

    url = f"{BASE_URL}/bot{TELEGRAM_BOT_TOKEN}/sendMessage"

    data = {
        "chat_id": chat_id,
        "text": message,
    }

    req = requests.post(url, data=data)
    if DEBUG:
        print(req.status_code)
        print(json.dumps(req.json(), indent=4))

    if req.status_code == 200:
        return {"status_code": req.status_code, "data": req.json()}
    else:
        return {"status_code": req.status_code, "data": f"Request Error: {req.text}"}
#
#
#
def sendLocation(chat_id:int=TELEGRAM_CHAT_ID, lat:float=None, long:float=None):
    """
    Send a geographic location to a specified Telegram chat.

    Args:
        chat_id (int, optional): The Telegram chat ID where the location will be sent.
                                 Defaults to TELEGRAM_CHAT_ID.
        lat (float, optional): Latitude value of the location. Must be provided for a valid request.
        long (float, optional): Longitude value of the location. Must be provided for a valid request.

    Returns:
        dict: { "status_code": int, "data": dict containing Telegram API response data or error message }
    """

    url = f"{BASE_URL}/bot{TELEGRAM_BOT_TOKEN}/sendLocation"

    data = {
        "chat_id": chat_id,
        "latitude": lat,
        "longitude": long,
        "protect_content": True,
    }

    req = requests.post(url, data=data)

    print(req.status_code)
    print(json.dumps(req.json(), indent=4))

    if req.status_code == 200:
        return {"status_code": req.status_code, "data": req.json()}
    else:
        return {"status_code": req.status_code, "data": f"Request Error: {req.text}"}
#
#
#
def getUpdates(offset:int=None):
    """
    Retrieve incoming updates for the bot.

    Args:
        offset (int): Offset to avoid duplicate updates.

    Returns:
        dict: { "status_code": int, "data": dict or error message }
    """

    url = f"{BASE_URL}/bot{TELEGRAM_BOT_TOKEN}/getUpdates"

    data = {
        "offset": offset,
        "limit": None,
        "timeout": None,
    }

    req = requests.post(url, data=data)

    # print(req.status_code)
    print(json.dumps(req.json(), indent=4))

    if req.status_code == 200:
        return {"status_code": req.status_code, "data": req.json()}
    else:
        return {"status_code": req.status_code, "data": f"Request Error: {req.text}"}
#
#
#
def query_ollama(message):
    url = "{}/api/generate".format_map(OLLAMA_URL)
    headers = {
        "Content-Type": "application/json"
    }

    data = {
        "model": OLLAMA_MODEL,
        "prompt": message,
        "stream": False
    }
    # print(data)

    req = requests.post(url, json=data, headers=headers)
    # print(req.text)

    if req.status_code == 200:
        return req.json().get("response")

    return ""
#
#
#
def llm_response():
    offset = None

    while True:
        try:
            updates = getUpdates(offset)

            if updates:
                messages = updates.get("result", list())

                if messages:
                    chat_id = messages[-1].get("message", dict()).get("chat", dict()).get("id", None)
                    update_id = messages[-1]["update_id"]
                    offset = update_id + 1
                    message = messages[-1].get("message").get("text")
                    # print(message)

                    # make ollama request
                    llm_response = query_ollama(message)

                    sendMessage(chat_id, llm_response)

        except KeyboardInterrupt:
            break

    return
#
#
#
import asyncio

interval = 0.1
allowed_updates = True

async def getUpdates_async(offset:int=None):
    path = "getUpdates"
    url = "{}/bot{}/{}".format(BASE_URL, TELEGRAM_BOT_TOKEN, path)

    data = {
        "offset": offset,
        "limit": None,
        "timeout": None,
    }

    req = requests.post(url, data=data)

    # print(req.status_code)
    # print(json.dumps(req.json(), indent=4))

    if req.status_code == 200:
        return req.json()
    
    return None
#
#
#
async def infinite_pooling():
    offset = None

    while True:
        try:
            updates = await getUpdates_async(offset)

            # to use plain function use:
            # loop = asyncio.get_event_loop()
            # future = loop.run_in_executor(None, getUpdates, offset)
            # updates = await future

            if updates:
                messages = updates.get("result", list())

                if messages:
                    chat_id = messages[-1].get("message", dict()).get("chat", dict()).get("id", None)
                    update_id = messages[-1]["update_id"]
                    offset = update_id + 1
                    message = messages[-1].get("message").get("text")
                    # print(message)

                    # make ollama request
                    llm_response = query_ollama(message)

                    sendMessage(chat_id, llm_response)
                
            if interval:
                await asyncio.sleep(interval)

        except Exception as ex:
            print(ex)
            continue
        
        except KeyboardInterrupt:
            return
#
#
#
if __name__ == "__main__":
    getMe()
    # print()
    # getUpdates()
    # print()
    sendMessage(message="i dont know")
    # sendMessage(message="i want a cookie")
    # print()
    # sendLocation()
    # print()
    # query_ollama("hello")
    # print()
    # llm_response()
    # print()
    # asyncio.run(infinite_pooling())
    # print()