def greet(name):
    return "Hello, " + name


def add(a, b):
    return a + b


def factorial(n):
    if n <= 1:
        return 1
    return n * factorial(n - 1)


class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y

    def distance(self):
        return (self.x ** 2 + self.y ** 2) ** 0.5


if __name__ == "__main__":
    print(greet("World"))
    print(add(1, 2))
    print(factorial(5))
