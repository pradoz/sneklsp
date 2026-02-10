# import statements
import boto3
from botocore.exceptions import ClientError
from aws_cdk import (
    App,
    aws_events as events,
)

# environment variables
NAMESPACE = os.environ["NAMESPACE"]
REGION = os.getenv("AWS_REGION", os.getenv("AWS_DEFAULT_REGION"))
PARTITION = os.environ.get(
    "AWS_PARTITION",
    boto3.Session().get_partition_for_region(region_name=REGION),
)

# basic expressions
x = 1
y = 2.5
z = "hello"

# binary operations
a = 1 + 2
b = 3 * 4 - 5
c = 10 / 2
d = 7 // 3
e = 8 % 3
f = 2 ** 10

# comparison
g = x < y
h = x == 1
i = y != z

# list/dict
my_list = [1, 2, 3]
my_dict = {"a": 1, "b": 2}
my_tuple = (1, 2, 3)

# function call
result = print("hello")
