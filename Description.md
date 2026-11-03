## Concept questions
A few sentences each, in your own words. These are about the code you wrote — point at your own implementation where a question asks you to, rather than repeating the textbook's answers.

**Under --features m4, late.brg (above) prints start and then stops at the whole assignment; under --features m5 it prints nothing and reports total, with expected ref<_>, found Int. Using your Assign arm and your unify, explain where the ref<_> comes from, why the error lands on total rather than on the whole assignment, and why expected and found come out in that order and not the reverse.**
The assign arm evaluates the expression to the left of the := to being an int. So when it is unified with a ref<_> (_ being a meta), it reports a mismatch. The error lands on total because that was the expression that had the incorrect type (my code does expr1.span() to accomplish this). expected is ref<_> because := implies there should be a ref on the left side.

**[] ++ [true] has type [Bool], but [] ++ [] is Ambiguous. Trace both through your concatenation arm: what type the left operand synthesizes, how the right operand is checked, and what type remains afterward. For [] ++ [], is it your concatenation arm or the driver's determination rule that reports Ambiguous (and when does the other one fire)? Why does a type have to be recorded for the determination rule to judge it?**
My concat arm will see that they are both lists (satisfying type safety), then will unify the sides together. unify will see that they are both lists and call unify on their inner types. For [] ++ [true] this will resolve the meta inside the left list to being bool, for [] ++ [] they are both meta, so there will still be an unresolved meta. The driver's determination rule reports Ambiguous when there are still unresolved metas (based on recorded types). The concatenation arm fires using the undetermined method when either side is a meta instead of a string or list. A type has to be recorded so the driver can keep track of what metas are still unresolved.

**In one body, id(1) and id("a") both check for a generic fn id<T>(x: T) -> T, but in |f| (f(1), f("a")) the second call is rejected. Point at the place in your Var arm where a type parameter becomes an inference variable, and explain why the first program checks. What would go wrong if that variable were made once per declaration instead of once per use? Why does the lambda's parameter f not get the same treatment?**
In Var if the var is a declaration, I match it to a function and get the generics. I create a new meta for each generic. Because there is only one meta for T once the declaration has been assigned to a variable, passing 1 and "a" causes a mismatch. The first program checks because the Var arm is called twice and each time it creates a new meta for the generic type. If it was once per declaration, the meta would be bound the first time the function is used and the generic property of the function would be lost. f doesn't get the same treatment because it is bound to a variable once before being used in the body of the lambda, this means the meta for the param is the same for both f(1) and f("a").

**{ let f = |x| x(x); () } is InfiniteType. Walk through what your Lambda and Call arms ask unify to do for x(x), and show where your occurs check fires. What would happen in your unify (and later, in resolve) if you left the occurs check out?**
In the call arm, we unify parameters with arguments. If the unify function sees that one side is a meta such as x, and the other side is a structural type with a meta inside, such as a function with x inside, (using a meta_in_meta function that checks structural types for metas with the same id) that has the same id, then it will report infinite type. If I left my occurs (meta_in_meta) check out of my unify fn, it would be stack overflow because the resolve would recurse trying to figure out x indefinitely (it would consider x to be a function in a function in a function in a...)

**fn f() -> Int { return 1; let y = []; println(y); 2 } checks, although nothing fixes the element type of y, while fn f() -> Int { return 1; 1 + true } is rejected. Using your Block arm, explain where you call dead_after, why that call has to come before the remaining statements are checked, and why the second program is still an error.**
In my block arm I check if the expression of the stmt I am currently checking is diverging (which would cause the rest of that block to be dead). If so, I call dead after on the span on the expression that was diverging. That call has to come before the remaining statments so that the checker knows that the remaining statements are in dead code. The second program is still an error because 1 + true is still a type mismatch of true not being an int even if its dead code.

## How you implemented it
**Walk through your Expr::Call arm and your unify from start to finish. For Call: how you get the callee's type, the three cases it can be in (a function type, an unsolved variable, anything else), how you check the arguments, and what finish_call does with the callee's bounds. For unify: the order of your cases and how you report the innermost disagreement inside compound types. Naming the provided tools you used (check_callee, fresh_meta, resolve, record, Ty::func, TyError, ?) is encouraged.**
- Call arm:
    - three cases
        - function type: when the call is on a function type, we check the arity of the parameters vs the arguments, then we unify each param with its cooresponding argument, then return the return type of the function
        - for an unsolved variable, we create a array of fresh metas for the params of the length of the arguments, we create a fresh meta for the return type, and we construct a fn type with the params and the return metas we just created. We then unify this fn type with the callee. Then we unify the params we created with the arguments passed to Expr::call, then we return the return value.
        - for anything else we return a NotAFunction error
    - how I check the arguments: I unify the arguments with the parameter types of the function
    - what finish_call does with the callee bounds: finish_call looks at the bounds ties them to the function arguments, so that later the checker can see if the function arguments satisfy the bounds.
- unify:
    - order of my cases:
        1. are they both metas? if they are different, solve one to the other
        2. is one a meta and the other isn't? check for infinite type then solve the meta to the non-meta's type
        3. both lists? unify inner type
        4. both tuples? compare lengths, then unify each cooresponding inner type
        5. both refernces? unify cell types
        6. both functions? unify return types, compare parameter lengths, unify cooresponding paramters
        7. both named? compare lengths of arguments, compare names, unify arguments
        8. anything else? compare directly and report mismatch if unequal.

**Describe how your check_expr enforces the nesting bound and keeps self.depth balanced when an arm fails, and whether you factored the shared ending of every arm (Chk-Sub, resolve, record) into a method of your own.**
my check_expr adds 1 to the depth at the start of the call, and before returning a value, decrements the depth (this is accomplished by making the match of expr that actually checks the expression a thunk, so that if there is a return, it won't bypass the decrementing of depth). Before depth is incremented, we compare the depth to max-nesting; if its greater than or equal, we return TooDeep.
The shared ending of every arm was rewritten many times, I probably could have made a method to do that.

**Point out anything that was tricky, a bug you fixed, or a design choice you made — and if you used an assistant, say what you had it do and how you checked its work.**
One thing that was tricky was figuring out what arm of my code was causing the final output of the checker. The solution was looking at the span of the output, comparing that to the bridger code, and figuring out what type of expression caused the output.
I did use an assistant. I had it explain to me why certain tests were failing. It didn't really do any work, it was more like a TA explaining why my code was failing so I could fix it myself.