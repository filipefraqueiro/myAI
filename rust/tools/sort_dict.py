#!/usr/bin/env python3
"""
Script to sort a Python dictionary by keys or values.
"""

def sort_dict_by_keys(d):
    """Sort a dictionary by its keys."""
    return {k: d[k] for k in sorted(d.keys())}

def sort_dict_by_values(d):
    """Sort a dictionary by its values."""
    return dict(sorted(d.items(), key=lambda item: item[1]))

if __name__ == "__main__":
    # Example dictionary
    my_dict = {
        "banana": 3,
        "apple": 1,
        "cherry": 2,
        "date": 4
    }

    print("Original dictionary:")
    print(my_dict)
    print()

    # Sort by keys
    sorted_by_keys = sort_dict_by_keys(my_dict)
    print("Sorted by keys:")
    for k, v in sorted_by_keys.items():
        print(f"  {k}: {v}")
    print()

    # Sort by values
    sorted_by_values = sort_dict_by_values(my_dict)
    print("Sorted by values:")
    for k, v in sorted_by_values.items():
        print(f"  {k}: {v}")
