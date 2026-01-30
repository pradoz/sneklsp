# intentional errors for testing parser diagnostics
# expected: ~20 parse errors

# error 1: missing closing paren in function def
def broken_function(x

# error 2: missing operand after operator
x = 1 +

# error 3: missing colon after if
if True
    pass

# error 4: missing colon after class
class Incomplete

# error 5: missing closing bracket
my_list = [1, 2, 3

# error 6: missing closing brace
my_dict = {"a": 1, "b": 2

# error 7: missing value in dict
bad_dict = {"key": }

# error 8: missing colon in dict
also_bad = {"key" 1}

# error 9: missing closing paren in call
print("hello"

# error 10: missing operand before operator
y = * 5

# error 11: missing colon after while
while True
    break

# error 12: missing colon after for
for i in items
    pass

# error 13: missing colon after elif
if x:
    pass
elif y
    pass

# error 14: missing colon after else
if x:
    pass
else
    pass

# error 15: missing expression after return
def bad_return():
    return +

# error 16: missing function name
def (a, b):
    pass

# error 17: missing class name
class :
    pass

# error 18: missing closing paren in tuple
my_tuple = (1, 2, 3

# error 19: unclosed string
bad_string = "hello

# error 20: missing target in for loop
for in items:
    pass

# error 21: missing iter in for loop
for x in:
    pass

# error 22: missing test in while
while:
    pass

# error 23: missing test in if
if:
    pass

# error 24: double operator
z = 1 + + 2

# error 25: missing argument in function call
result = len(

# valid code at the end to ensure parser recovers
valid_x = 42
valid_y = valid_x + 1

def valid_function(a, b):
    return a + b

class ValidClass:
    def __init__(self):
        self.value = 0
lass Incomplete
