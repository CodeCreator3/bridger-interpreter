## Concept questions
A few sentences each, in your own words. These are about the code you wrote — point at your own implementation where a question asks you to, rather than repeating the textbook's answers.

**What does it mean for an expression to be stuck, as opposed to evaluating to a value?**
That means there is no way for the program to continue. In practice it means that the premise of an expression isn't met. In my code that is when the match statement checks the premise and returns an error value if the premise is not met.

**Explain in terms of your method's return type: what does eval_expr return in each case (the Ok/Err shape), and for a stuck expression, which RuntimeError variant and expected/found types does it carry?**

Note: there are some cases that aren't covered currently by my program. Those are currently filled in by panic!().
- E-Lit: currently no Err returned. Only returns Ok(whatever literal is being returned)
- Unary: if the op is neg, checks that the expr is an int and returns a type error if not. if the op is not, checks that the expr is a bool and returns type error if not. If types match their operation, an ok result is outputted
- Binary: checks types (for +,-,*,/,%,>=,<=,<,> makes sure it is int, for &&,|| makes sure it is bool; will return type error if not), for / and % makes sure divisor is !=0 (will return div by 0 error if not)
- Tuple: simply creates a vec, pushes values to it, then converts to tuple. No errors currently being returned.
- List: simply creates a list and pushes values to it. No errors currently being returned.
- Proj: will raise no such field if the index is out of bounds or the expr that should be a tuple is not a tuple.

**Walk through how the ? operator behaves in one of your arms: when you write self.eval_expr(sub, env)?, what happens if that subexpression is stuck, and what does it save you from writing by hand? What would the same arm look like without ??**

The ? operator is syntactic sugar for returning the Value if the Result<Value, Control> is a value and Err if not.
If the subexpression is stuck, it will return an Err.
It would look something like this (Pseudocode):
```rust
match result{
    Value(v) => v,
    Err(e) => e,
}
```

**In your and/or implementation, how do you avoid evaluating the right operand once the left operand has already decided the answer? Point to the lines that do it, name the Rust control flow you used, and say why you could not evaluate both operands first and then combine them.**
That is called short-circuiting. In line 252 of eval.rs (inside my logical_op function), we check if the left expression is a bool. Then if the op is an or and the left side is true, we return true without even evaluating the right side. If the op is an and and the left side is false, we return false without evaluating the right side. I could not evaluate both operands first because this would ignore the short-circuiting rule that we have for bridger.

**Why does your integer arithmetic wrap rather than overflow-panic, and which specific operations (or method calls) in your code produce that behavior?**
To prevent overflow panic, I used Rust's built in wrapping functions. (wrapping_add, wrapping_sub, wrapping_mul, wrapping_div, wrapping_rem)

**When your code reports a stuck expression, which node's span does the error carry, and how does your code get that span? Why is blaming that node more useful to a user than blaming, say, the whole expression?**
When my code reports a stuck expression, it carries the span from the caller. It gets that span as a parameter. This is useful because it tells the user where the error came from.

## How you implemented it

**Walk through your Expr::Binary arm: how do you evaluate the operands, dispatch on the BinOp, build the result value, and report a type error? Naming the provided tools you used (self.eval_expr, type_of, the wrapping operations, RuntimeError, .into()/?) is encouraged.**
For the binary arm, I first check the operation. I have one for arithmatic (+-*), one for division and modulo, one for logical operators, one for comparison, one for concat, and one for cons.
In each case, I evaluate the expressions, I check that the type matches the operation by putting the operands through a match statement and returning a type error in the case of an incorrect type.
Once I have the correct type, I unpack the value, do the operation, and construct the output Value.

Tools I used:

- rust arithmetic wrapping functions
- panic!() (in cases where I haven't implemented all possibilities)
- into (when concatinating strings, I used into to pack the new string into an Rc)
- Runtime Errors (I used type errors for incorrect types and div by 0 errors for division and modulo where divisor by 0)
- reference and deref
- type_of (allows me to report an incorrect type without having to enumerate each possible incorrect type in my match statements)

**Point out anything that was tricky, a bug you fixed, or a design choice you made — and if you used an assistant, say what you had it do and how you checked its work.**

One thing that was tricky was taking every type incompatibility into account, especially for binary operations. There were so many combinations of incorrect types that had to be accounted for that it became exhausting writing each one out for each function. After some investigation, I realized I could use type_of to handle all incorrect types at once, which cut down on a lot of code and complexity.
I used Microsoft Copilot to give me suggestions about how to handle certain situations. For example, I asked it how to do wrapping arithmetic operations in rust. It told me the functions to use, I implemented them and tested the code (cargo m1) to make sure it worked.