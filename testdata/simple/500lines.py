# basic variable assignments
x = 1
y = 2
z = 3
name = "hello"
pi = 3.14159
active = True
inactive = False
nothing = None

# arithmetic expressions
sum_val = 1 + 2
diff = 10 - 5
product = 3 * 4
quotient = 20 / 4
floor_div = 17 // 3
remainder = 17 % 3
power = 2 ** 8

# compound expressions with precedence
expr1 = 1 + 2 * 3
expr2 = (1 + 2) * 3
expr3 = 2 ** 3 ** 2
expr4 = 10 - 5 - 2
expr5 = 100 / 10 / 2

# unary operators
neg = -42
pos = +42
inv = ~0

# bitwise operations
bit_or = 1 | 2
bit_xor = 3 ^ 1
bit_and = 7 & 3
left_shift = 1 << 4
right_shift = 16 >> 2

# comparison operations
eq = x == 1
not_eq = x != 2
less = x < 10
less_eq = x <= 10
greater = x > 0
greater_eq = x >= 0

# boolean operations
bool_and = True and False
bool_or = True or False
bool_not = not True

# identity tests
is_none = nothing is None
is_not_none = x is not None

# list literals
empty_list = []
numbers = [1, 2, 3, 4, 5]
mixed = [1, "two", 3.0, True, None]
nested_list = [[1, 2], [3, 4], [5, 6]]
trailing_comma = [1, 2, 3,]

# tuple literals
empty_tuple = ()
single = (1,)
pair = (1, 2)
triple = (1, 2, 3)
nested_tuple = ((1, 2), (3, 4))

# dictionary literals
empty_dict = {}
simple_dict = {"a": 1, "b": 2, "c": 3}
int_keys = {1: "one", 2: "two", 3: "three"}
nested_dict = {"outer": {"inner": 42}}
dict_trailing = {"x": 1, "y": 2,}

# subscript access
first = numbers[0]
last = numbers[4]
nested_access = nested_list[0][1]
dict_access = simple_dict["a"]

# attribute access
length = numbers.append
method_result = "hello".upper

# function calls
result = print("hello")
multi_arg = print("a", "b", "c")
no_args = print()
nested_call = len(str(123))

# chained operations
chained = [1, 2, 3][0]
method_chain = "hello".upper().lower

# augmented assignment
counter = 0
counter += 1
counter -= 1
counter *= 2
counter /= 2
counter %= 10

# simple function definitions
def empty_func():
    pass

def return_none():
    return

def return_value():
    return 42

def single_param(x):
    return x

def two_params(a, b):
    return a + b

def three_params(x, y, z):
    return x + y + z

def with_default(x, y=10):
    return x + y

def multiple_defaults(a, b=1, c=2):
    return a + b + c

def typed_param(x: int):
    return x * 2

def typed_return(x) -> int:
    return x

def fully_typed(x: int, y: int) -> int:
    return x + y

def mixed_typed(a: int, b, c: str):
    return a

# function with body
def add_numbers(a, b):
    result = a + b
    return result

def multi_statement(x):
    y = x * 2
    z = y + 1
    return z

def with_locals(n):
    a = 1
    b = 2
    c = 3
    return a + b + c + n

# nested function calls in body
def process(data):
    length = len(data)
    return length

# if statements
def simple_if(x):
    if x:
        return 1
    return 0

def if_else(x):
    if x > 0:
        return 1
    else:
        return 0

def if_elif(x):
    if x > 0:
        return 1
    elif x < 0:
        return -1
    else:
        return 0

def if_elif_chain(x):
    if x == 1:
        return "one"
    elif x == 2:
        return "two"
    elif x == 3:
        return "three"
    else:
        return "other"

def nested_if(x, y):
    if x > 0:
        if y > 0:
            return 1
        else:
            return 2
    else:
        return 3

# while loops
def while_simple(n):
    i = 0
    while i < n:
        i += 1
    return i

def while_with_break(n):
    i = 0
    while True:
        i += 1
        if i >= n:
            break
    return i

def while_with_continue(n):
    i = 0
    total = 0
    while i < n:
        i += 1
        if i % 2 == 0:
            continue
        total += i
    return total

def while_else(n):
    i = 0
    while i < n:
        i += 1
    else:
        return i
    return 0

# class definitions
class Empty:
    pass

class WithAttribute:
    x = 10

class WithMethod:
    def get_value(self):
        return 42

class WithInit:
    def __init__(self):
        pass

class WithInitParams:
    def __init__(self, x, y):
        self.x = x
        self.y = y

class WithMultipleMethods:
    def __init__(self, value):
        self.value = value

    def get(self):
        return self.value

    def set(self, new_value):
        self.value = new_value

class WithInheritance(Empty):
    pass

class MultipleBase(WithAttribute, WithMethod):
    pass

class Calculator:
    def __init__(self):
        self.result = 0

    def add(self, x):
        self.result += x
        return self

    def subtract(self, x):
        self.result -= x
        return self

    def multiply(self, x):
        self.result *= x
        return self

    def get_result(self):
        return self.result

# import statements
import os
import sys
import json

# import with alias
import os as operating_system
import json as j

# from imports
from os import path
from sys import exit
from collections import OrderedDict

# from import with alias
from os import path as p
from json import dumps as to_json

# multiple imports from same module
from os import getcwd, listdir
from sys import argv, exit, path

# relative imports
from . import module
from .. import parent
from .sibling import something
from ..parent import other

# complex expressions in statements
def complex_expressions():
    a = 1 + 2 * 3 - 4 / 2
    b = (1 + 2) * (3 + 4)
    c = [1, 2, 3][0] + [4, 5, 6][1]
    d = {"a": 1}["a"] + {"b": 2}["b"]
    e = len([1, 2, 3]) * 2
    return a + b + c + d + e

def call_with_expressions():
    print(1 + 2)
    print([1, 2, 3][0])
    print(len("hello"))
    return None

def assignments_variety():
    x = 1
    y = x + 1
    z = y * 2
    a = [x, y, z]
    b = {"x": x, "y": y}
    c = len(a)
    return c

# expression statements
1 + 2
print("hello")
[1, 2, 3]
{"a": 1}

# control flow in complex scenarios
def complex_control(x, y, z):
    if x > 0:
        if y > 0:
            if z > 0:
                return 1
            else:
                return 2
        else:
            return 3
    else:
        return 4

# deeply nested structures
deep_list = [[[1, 2], [3, 4]], [[5, 6], [7, 8]]]
deep_dict = {"a": {"b": {"c": {"d": 1}}}}
deep_access = deep_list[0][1][0]
deep_dict_access = deep_dict["a"]["b"]["c"]["d"]

# mixed data structures
mixed_nested = [{"a": [1, 2]}, {"b": [3, 4]}]
dict_of_lists = {"nums": [1, 2, 3], "strs": ["a", "b", "c"]}
list_of_dicts = [{"x": 1}, {"y": 2}, {"z": 3}]

# complex function bodies
def fibonacci(n):
    if n <= 0:
        return 0
    if n == 1:
        return 1
    a = 0
    b = 1
    i = 2
    while i <= n:
        temp = a + b
        a = b
        b = temp
        i += 1
    return b

def factorial(n):
    if n <= 1:
        return 1
    result = 1
    i = 2
    while i <= n:
        result *= i
        i += 1
    return result

def gcd(a, b):
    while b != 0:
        temp = b
        b = a % b
        a = temp
    return a

def is_prime(n):
    if n < 2:
        return False
    if n == 2:
        return True
    if n % 2 == 0:
        return False
    i = 3
    while i * i <= n:
        if n % i == 0:
            return False
        i += 2
    return True

class LinkedList:
    """linked-list data structure"""

    def __init__(self):
        self.head = None
        self.size = 0

    def is_empty(self):
        return self.head is None

    def get_size(self):
        return self.size

    def clear(self):
        self.head = None
        self.size = 0

class Stack:
    def __init__(self):
        self.items = []

    def is_empty(self):
        return len(self.items) == 0

    def push(self, item):
        self.items.append(item)

    def peek(self):
        if self.is_empty():
            return None
        return self.items[len(self.items) - 1]

class Queue:
    """ queue wrapper """

    def __init__(self):
        self.items = []

    def is_empty(self):
        return len(self.items) == 0

    def enqueue(self, item):
        self.items.append(item)

    def size(self):
        return len(self.items)

class TreeNode:
    """ binary tree node """

    def __init__(self, value):
        self.value = value
        self.left = None
        self.right = None

    def is_leaf(self):
        return self.left is None and self.right is None

    def has_left(self):
        return self.left is not None

    def has_right(self):
        return self.right is not None

# main guard pattern
if __name__ == "__main__":
    print("Running main")
    x = 42
    y = add_numbers(1, 2)
    print(y)

    fib_result = fibonacci(10)
    print(fib_result)

    fact_result = factorial(5)
    print(fact_result)

    calc = Calculator()
    calc.add(10)
    calc.multiply(2)
    result = calc.get_result()
    print(result)
