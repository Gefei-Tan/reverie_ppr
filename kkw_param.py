import operator as op
from functools import reduce

# compute n \choose r
def ncr(n, r):
    r = min(r, n-r)
    numer = reduce(op.mul, range(n, n-r, -1), 1)
    denom = reduce(op.mul, range(1, r+1), 1)
    return numer // denom  # or / in Python 2

# number of parties simulated
n = 16

# statistical security
target = 0.5**60
lst = []

# soundness error of KKW
def err_sg(M, tau, n, rho):
    a = ncr(rho, M-tau)
    b = ncr(M, M-tau)
    c = n**(rho-M+tau)
    try:
        d = a/b
        e = d/c
    except OverflowError:
        return 0.0
    return e

# try all possible \rho given fixed M, \tau, n
def err(M, tau, n):
    base = 0.0
    for rho in range(M-tau+1, M+1):
        ret = err_sg(M, tau, n, rho)
        if ret > base:
            base = ret
    if base < target:
        #print(str(M) + " " + str(tau) + " " + str(base))
        lst.append(tuple((M,tau)))

# try all possible M and \rho in given range
for M in range(64, 512, 8):
    #print(M)
    for tau in range(8, 50, 8):
        err(M, tau, n)

print("finish computing")

# select the best result
# prioritize on \tau
min_tuples = tuple((10000,10000))
for tp in lst:
    if tp[1] < min_tuples[1]:
        min_tuples = tp
    if tp[0] < min_tuples[0] and tp[1] == min_tuples[1]:
        min_tuples = tp

print(min_tuples)
