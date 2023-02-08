#!/usr/bin/env python
import os.path
import re
import sys 
import matplotlib.pyplot as plt
from quantiphy import Quantity
from matplotlib.ticker import FuncFormatter

# Define a filename.
filename = sys.argv[1]

# Protocol used
protocol = sys.argv[2] 
assert protocol in ["udp", "tcp"]

# Open file
if not os.path.isfile(filename):
    print('File does not exist.')
else:
    with open(filename) as f:
        content = f.read().splitlines()



########################## Parser ##################################

# Flags
IS_SERVER = False
FOUND_HEADERS = False
FOUND_SUMMARY_HEADERS = False
READ_SUMMARY_ELEMENTS = False

# Return values
data = {
    "title" : "undefined",
    "headers" : {

    },
    "headers_summary" : {

    },
}

# Result list
res = []

print("------------------------------------")
print("Parsing...")
print("------------------------------------")

# Begin parsing        
for line in content:

    ## Find "Server is now listening on..." and catch "(test #X)". When found, continue.
    if not IS_SERVER:
        title = re.search("\([^)]*\)", line)
        if title != None:
            data.update({"title" : title.group()})
            IS_SERVER = True

    ## Find first ID row
    if not FOUND_HEADERS:   
        if line.find("ID") != -1:
            headers = line.split()[2:]
            for header in headers:
                data["headers"].update({header : []})

            FOUND_HEADERS = True
            continue
        else:
            continue
      
    ## Read data
    if not FOUND_SUMMARY_HEADERS:
        ## Find second ID row
        if not line.find("ID") != -1:
            ## Reading data..
            values = line.split()[2:]
            if (values[0] == "-"):
                continue
            i = 0
            for list in data["headers"].values():
                try:
                    list.append(values[i] + " " + values[i + 1])
                    i += 2
                except:
                    continue
                continue

        else: ## Not reading more data, found summary header and read headers...
            headers = line.split()[2:]
            for header in headers:
                data["headers_summary"].update({header : []})
            FOUND_SUMMARY_HEADERS = True
            continue    

    ## Read summary
    values = line.split()[2:]
    i = 0
    for list in data["headers_summary"].values():
        try:
            list.append(values[i] + " " + values[i + 1])
            i += 2
        except:
            list.append(values[i])

    ## End        
    if FOUND_SUMMARY_HEADERS:
        
        FOUND_HEADERS = False
        FOUND_SUMMARY_HEADERS = False
        READ_SUMMARY_ELEMENTS = False
        res.append(data) # Add data to the result list and reset data
        print("** Found test " + data.get("title") + " **")
        data = { # Reset data
            "title" : "undefined",
            "headers" : {

            },
            "headers_summary" : {

            },
        }

print("------------------------------------")
print("Done! Converting data to plots...")
print("------------------------------------\n")
########################## End of Parser ##################################





########################## Plotter ##################################

# variables for summary graph
summary_transfer_fig, summary_transfer_ax = plt.subplots()
summary_bitrate_fig, summary_bitrate_ax = plt.subplots()
labels = []
summary_transfer = []
summary_bitrate = []
width = 0.35

# define the axis formatting routines
bitrate_formatter = FuncFormatter(lambda v, p: str(Quantity(v, 'bits/sec')))
byte_formatter = FuncFormatter(lambda v, p: str(Quantity(v, 'Bytes')))
time_formatter = FuncFormatter(lambda v, p: str(Quantity(v, 's')))

# format axis for summary graphs
summary_transfer_ax.yaxis.set_major_formatter(byte_formatter)
summary_bitrate_ax.yaxis.set_major_formatter(bitrate_formatter)

summary_transfer_ax.autoscale(True, axis="y")


for i, test in enumerate(res):
    
    title = "test #{} - {}".format((i+1), protocol)                             # title
    title = title + " (Server)" if IS_SERVER else title + " (Client)"
    t = []                                                                      # time
    transfer =  []                                                              # transfers, list with [Quantity], see Quantiphy
    bitrate =   []                                                              # bitrate, list with [Quantity], see Quantiphy

    ## time arrives in the format ['FLOAT-FLOAT sec', ..]
    for element in test["headers"].get("Interval"):
        temp = element.split()      # ['FLOAT-FLOAT', 'sec']
        temp = temp[0].split("-")   # ['FLOAT', 'FLOAT]
        t.append(Quantity(temp[1], "s"))
        
    for element in test["headers"].get("Transfer"):
        transfer.append(Quantity(element))

    for element in test["headers"].get("Bitrate"):
        bitrate.append(Quantity(element))

    # last element is always pretty messed up, so let's just remove it...
    transfer.pop()
    bitrate.pop()
    t.pop()

    # construct transfer
    fig, ax = plt.subplots()
    ax.plot(t, transfer)
    ax.set(title=title + " - TRANSFER")
    ax.yaxis.set_major_formatter(byte_formatter)
    ax.xaxis.set_major_formatter(time_formatter)
    ax.grid()

    # construct bitrate
    fig, ax = plt.subplots()
    ax.plot(t, bitrate)
    ax.set(title=title + " - BITRATE")
    ax.yaxis.set_major_formatter(bitrate_formatter)
    ax.xaxis.set_major_formatter(time_formatter)
    ax.grid()

    # summmary handler
    labels.append(title)
    summary_transfer.append(Quantity(test["headers_summary"].get("Transfer")[0]))
    summary_bitrate.append(Quantity(test["headers_summary"].get("Bitrate")[0]))

# construct summary graphs
summary_bitrate_ax.bar(labels, summary_bitrate, width)
summary_bitrate_ax.set_title("Summary of bitrate")
summary_transfer_ax.bar(labels, summary_transfer, width)
summary_transfer_ax.set_title("Summary of transfers")

plt.show()