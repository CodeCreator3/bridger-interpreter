## Concept questions
A few sentences each, in your own words. These are about the code you wrote — point at your own implementation where a question asks you to, rather than repeating the textbook's answers.

**In { let y = 1; let f = |x| x + y; let y = 100; f(0) } the result is 1, not 101. Walk through what your Expr::Lambda arm stores in the closure, which environment your Expr::Call arm extends with the parameter x, and why that gives 1. What would the result be if your Call arm extended the caller's environment instead, and what is the name for the scoping discipline your interpreter implements?**

The Lambda arm stores the current environment in the closure for use when called. The call arm extends the closure's stored environment with the parameter x. That gives 1 because the closure env was stored when the lambda was created and the outer env had y=1. If call extended the callers environment, the result would be 100 (0+100). The scoping discipline is lexical scoping.

**Your Call arm evaluates the operator, the arguments, and checks the arity, in some order. Say the order your code uses, and use it to explain why 5(print(1)) is stuck with NotAFunction without printing anything, while (|x, y| 0)(print(1)) prints 1 and then is stuck with ArityMismatch. Which span do both errors carry?**

First, it evaluates the operator, then it evaluates the arguments, then it checks the arity, 5(print(1)) would be stuck before reaching the argument because my code checks that the operator is a function before evaluating the arguments. (|x, y| 0)(print(1)) gets a valid operator, then evaluates the arguments (printing 1), then checks the arity which is what causes it to be stuck. Both have the span of the caller.

**At M3, nothing you wrote caught Control::Return. Now your Call arm does. Show how your arm turns Err(Control::Return(v)) into the call's value while letting a Control::Raise through, and explain why you must not apply ? to the body's result before calling leave_call. What two things would go wrong if you did?**

My code matches the result of the call code to Result::Err, if it isn't an Err, the Value is passed on. If it is an Err, the control is matched to Return, if it isn't return, the Err is passed on. If it is a Return, the value of the return is converted into a Result::Value and passed on. You don't apply ? to the body's result before leave_call because we want the call arm to output a Result, not a Value and because we don't want to skip the leave_call method (which keeps track of stack depth).

**fold(range(0, 1000000), 0, |a, x| a + x) runs to completion, while the recursive sum_to(1000000) is a StackOverflow, even though both call a Bridger function a million times. Explain the difference in terms of how many calls are in progress at once and what your enter_call/leave_call pair counts. How does fold's callback reach your Call arm at all, given that fold is a native?**

Fold runs like a for loop where each function call is seperate from each other. Recursive calls happen inside of each other so you can end up with many functions running at the same time. Both natives and bridger functions are considered Closures. When an Expr in the AST is a Closure (which happens when the fold function is used), it is sent down the call branch via the overall match function.

**A top-level fn fact can call itself, but let fact = |n| if n == 0 { 1 } else { n * fact(n - 1) }; inside main is stuck with an unbound variable. Using what your Lambda arm captures and what the provided driver does for top-level functions, explain why the two behave differently.**

The lambda arm captures the env before the new binding is bound. The provided driver binds the function to the environment before storing the closure.

## How you implemented it
**Walk through your Expr::Call arm from start to finish: how you destructure it, how you evaluate the operator and arguments, how you dispatch on Closure::Bridger versus Closure::Native, how you build the body's environment, and how you handle the body's result. Then describe your Expr::Lambda arm in a sentence or two. Naming the provided tools you used (self.eval_expr, env.extend, enter_call, leave_call, eval_prelude, type_of, RuntimeError, .into()/?) is encouraged.**

Expr::Call arm:
1. call enter_call with ? to pass on StackOverflow
2. evaluate operator to value
3. create mutable result variable that will be used to store output
4. check if operator is a closure (if not: NotAFunction)
5. evaluate arguments
6. if closure is native, call eval_prelude on args and save result to result variable
7. if closure is bridger:

{

8. check lengths of param vec and args vec to make sure they match (or else: ArityMismatch)
9. clone the closure's environment
10. extend the cloned env with parameters as names and arguments as values
11. evaluate the body with the new environment (save to result variable)

}

12. if the result variable is a Control::Return, convert to Value and save to result variable
13. output result variable

**Say what, if anything, you changed in your M1–M3 arms for this milestone — for example, to make == on a function stuck, or to fix an evaluation-order problem a whole-program test exposed — and why.**

I ended up having to call the uncomparable fn on values to check whether they could be compared for == and !=. If not, I return a NotComparable error.

**Point out anything that was tricky, a bug you fixed, or a design choice you made — and if you used an assistant, say what you had it do and how you checked its work.**

One thing that was tricky was trying to match the closure value in call. When I tried to dereference it, Rust got mad because apparently it couldn't move out of an Rc. The solution ended up being using the as_ref fn to get Rust to let me access the closure.

That was one example of where I used an AI assistant to help me get my program working. I checked its work by running the program and confirming I stop getting the "cannot move out of an Rc" error. The other time I used AI for m4 was when the "Operands are evaluated left to right" hidden check was failing upon submission. It turns out that checking for closures in the comparison arm wasn't enough, functions could be hidden in data (like lists and tuples). That is why the AI suggested switching to the uncomparable fn for checking whether datatypes could be comparable.
