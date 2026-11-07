import plyer
import requests
# def get_device_status():
#     # Automatically invokes PyJNIus on Android or PyObjUS on iOS
#     battery_info = plyer.battery.status
    
#     return {
#         "percentage": battery_info.get("percentage", 0),
#         "is_charging": battery_info.get("is_charging", False),
#     }

def get_cpu():
    """
    Get device CPU details using plyer

    Args:

    Returns:
        dict: { "numa": int, "sockets": int, "physical": int}
    """
    return {
        "numa": plyer.cpu.numa,
        "sockets": plyer.cpu.sockets,
        "physical": plyer.cpu.physical
    }
    
def make_web_request(url:str=None, method:str="GET"):
    """
    Make a web request to a geven URL

    Args:
        url
        method
    Returns:
        dict: { "status_code": int, "response": str}
    """
    
    req = requests.request(method=method, url=url)
    
    return {
        "status_code": req.status_code,
        "response": req.text,
    }

if __name__ == "__main__":
    # get_device_status()
    # a = get_cpu()
    a = make_web_request("https://www.mvg.de/api/bgw-pt/v3/lines")
    print(a)