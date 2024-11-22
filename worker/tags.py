import json

import redis

r = open("tags.json", "r")

j = json.load(r)

client = redis.StrictRedis(host='localhost', port=6379, db=1)

for i in j:
	client.set(i['mercacc'], i['tag'])