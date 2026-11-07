import requests
import pyperclip
import os
# import mysql.connector
import json
# from googlesearch import search
from duckduckgo_search import DDGS
from bs4 import BeautifulSoup
import subprocess
import shlex
from dotenv import load_dotenv

from tools.yahoo_finances import get_info
from tools.check_headers import check_secure_headers
from tools.api_make_requests import make_request
from tools.telegram_bot import getMe, sendMessage, getUpdates
from tools.api_weather import current, forecast

#
#
#
load_dotenv()
#
#
#
def create_file(filename:str=None, content:str=""):
    """Creates a file with specified filename and adds the content

    # Args:
    filename (str): the new file name
    content (str): content to be added to the file name    
    """

    if filename:
        with open(filename, "w") as fp:
            fp.write(content)

        return f"{filename} successfully created."

    return f"{filename} not provided."
#
#
#
def read_file(filename:str=None):
    """Reads a file with specified filename

    # Args:
    filename (str): the new file name
    content (str): content to be added to the file name    
    """

    if filename:
        with open(filename, "r") as fp:
            data = fp.read()

        return data

    return f"{filename} not provided."
#
#
#
def copy_to_clipboard(text:str):
    """Copy/Add the text to clip board

    # Args:
    text (str): the text to be copied to clipboard
    """

    pyperclip.copy(text)

    return f"{text} successfully copied to the clipboard."
#
#
#
def sql_request(query:str, host:str, user:str, password:str, database:str=None, port:int=3306):
    try:
        con = mysql.connector.connect(host=host, user=user, password=password, database=database, port=port, use_pure=True)
        print(con)

    except Exception as ex:
        print(ex)
        return None

    cur = con.cursor()

    cur.execute(query)

    rows = cur.fetchall()

    data = list()

    for row in rows:
        print(row)
        data.append(row)

    con.close()

    return data
#
#
#
def duckduckgo_web_search(query:str=None, search_type:str="text", max_results:int=10, verbose:bool=False):
    """
    Performs a DuckDuckGo search using the DDGS API.

    Parameters:
        query (str): The search query string. (Required)
        search_type (str): The type of search. Options are 'text', 'image', 'video', or 'news'. Default is 'text'.
        max_results (int): Maximum number of results to return. Default is 10.
        verbose (bool): If True, prints the results in pretty JSON format. Default is False.

    Returns:
        list or None: A list of search result dictionaries, or None if an error occurs or the query is invalid.
    """

    if not query:
        print("[+] Query input is required")
        return None

    try:
        headers = {
            "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36",
        }

        ddgs = DDGS(proxy=None, timeout=10, headers=headers)

        if search_type.lower() == "text":
            results = ddgs.text(
                keywords=query,
                region="wt-wt",
                safesearch="on",
                # timelimit="m",
                max_results=max_results,
                backend="html"
            )

        elif search_type.lower() == "image":
            results = ddgs.images(
                keywords=query,
                region="wt-wt",
                safesearch="off",
                size=None,
                color="Monochrome",
                type_image=None,
                layout=None,
                license_image=None,
                max_results=max_results,
            )

        elif search_type.lower() == "video":
            results = ddgs.videos(
                keywords=query,
                region="wt-wt",
                safesearch="off",
                timelimit="w",
                resolution="high",
                duration="medium",
                max_results=max_results,
            )

        elif search_type.lower() == "news":
            results = ddgs.news(
                keywords=query,
                region="wt-wt",
                safesearch="off",
                timelimit="m",
                max_results=max_results
            )

        if verbose:
            print(json.dumps(results, indent=4))
        
        return results

    except Exception as ex:
        print(ex)
        return f"Request failed: {ex}"
#
#
#
def google_web_search(query:str=None, search_type:str="text", num_results:int=5, verbose:bool=False):
    """
    Performs a Google web search using the `googlesearch` package and returns a list of result URLs.

    Parameters:
        query (str): The search query string. Required.
        search_type (str): Type of search (currently only 'text' is supported). Default is 'text'.
        num_results (int): Number of results to return. Default is 10.
        verbose (bool): If True, prints the results in pretty JSON format. Default is False.

    Returns:
        list of str: A list of result URLs if successful.
        str: An error message if the request fails.
    """
    
    if not query:
        print("[+] Query input is required")
        return "Error: Search query is required."
    
    try:
        results_ = search(query, num_results=num_results, unique=True, lang="en",  region=None, safe=None, advanced=True,  ssl_verify=True)

        results = list()
        for r in list(results_)[1:]:
            results.append({
                "url": r.url,
                "title": r.title,
                "description": r.description,
                "content": web_request(url=r.url),
            })

        if verbose:
            print(json.dumps(results, indent=4))

        return results

    except Exception as ex:
        if verbose:
            print(ex)
        return f"Request failed: {ex}"
#
#
#
def web_request(url:str=None):
    """
    Sends an HTTP GET request to the specified URL and extracts the <body> content.

    Parameters:
        url (str): The target URL. Required.

    Returns:
        str: The inner HTML of the <body> tag if the request is successful,
             or an error message otherwise.
    """

    if not url:
        return None

    headers = {
        "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36",
    }

    req = requests.get(url=url, headers=headers, verify=True, timeout=10, allow_redirects=True)

    if req.status_code == 200:
        soup = BeautifulSoup(req.text, "html.parser")
        body = soup.body
        text = body.decode_contents()

        with open("request.html", "wb") as fp:
            fp.write(text.encode())

        return text

    else:
        return f"Request failed with code {req.status_code} - {req.reason}"
#
#
#
def run_shell_command(cmd:str, timeout:int=10, verbose:bool=False):
    """
    Executes a shell command and returns the output or error.

    Parameters:
        cmd (str): The shell command to execute.
        timeout (int): Timeout in seconds for the command. Default is 10.
        verbose (bool): If True, prints the command output or error. Default is False.

    Returns:
        str: The standard output or error from the command.
    """
    if not cmd:
        return "Error: Command is required."

    try:
        use_shell = True
        args = shlex.split(cmd)

        result = subprocess.run(
            args,
            shell=use_shell,
            capture_output=True,
            text=True,
            timeout=timeout
        )

        output = result.stdout.strip() if result.stdout.strip() else result.stderr.strip()

        if verbose:
            print(result)
            print(f"Command output:\n{output}")

        return output

    except subprocess.TimeoutExpired:
        return f"Error: Command timed out after {timeout} seconds."

    except Exception as ex:
        return f"Error: {str(ex)}"
#
#
#
if __name__ == "__main__":
    pass
