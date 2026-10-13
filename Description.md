## Concept questions
A few sentences each, in your own words. These are about the code you wrote — point at your own implementation where a question asks you to, rather than repeating the textbook's answers.

**Your eval_expr returns Result<Value, Control>, and a return travels as Err(Control::Return(v)) — the same Err side an error uses. In your Expr::Return arm, how do you build that value, and why carry a return as an Err rather than an ordinary Value? How do the two channels — Control::Return and Control::Raise — stay distinct, and what catches Control::Return at this milestone?**
I built the `Err::Return` value by calling `Err(Control::Value(v))`. This is necessary so that the value is brought straight to the caller without executing the rest of the code, just like an actual error would be. `Return` and `Raise` are different enums of the Control type, this distinction can be detected later. However, in this milestone there is no function to catch return.

**Walk through your assignment arm e1 := e2: in what order do you evaluate the two sides, which one must be a Value::Ref(loc), which self.store operation you call, and what value the form returns. For the stuck case — assigning to something that is not a reference — name the RuntimeError variant you build and how you form the expected type.**
I evaluate the left side first to make sure it is a Ref value (pointer). Then I evaluate the second expression and write it to the store a that location. `e1` must be `Value::Ref(loc)`. The `self.store` operation I call is `write`. The form returns `Value::Unit`. The `RuntimeError` is `TypeError`. I form the expected type using this:
```Rust
expected: Ty {
                    kind: TyKind::Ref(Rc::new(Ty {
                        kind: TyKind::Int,
                        span: None,
                    })),
                    span: None,
                }
```
Note that the inner type of the expected Ref is Int, because it is actually ambiguous and their's no way to set TyKind to ambiguous (so I am limited by the TyKind enum).

**The Mutation chapter's rules thread a store in and a store out, yet your interpreter keeps one self.store and mutates it in place. Explain, using let a = ref 1; let b = a; b := 9; deref a ⇒ 9, why aliasing works: what ref allocated, what b = a copied, and why writing through b is visible through a.**
ref allocated `a` as a reference to `1`. `b = a` copied that reference to `b`. Writing through `b` is visible to `a` because they both point to the same location that can be updated via either pointer.

**Your while arm is a host loop and your for arm walks a list. For each, say what value the body produces and what you do with it, what the loop as a whole evaluates to, and — for for — how env.extend(x, item) gives the loop variable a fresh binding each turn. Then explain how a return inside either body escapes the loop, referring to the ? in your arm.**
The body of while produces a boolean each loop and if the boolean is true, I continue the loop, if not, I exit the loop and return Unit (meaning the loop will eventually return Unit provided it isn't an infinite loop). The body of for binds that element of the list to the provided name before running the main for loop expression with that new environment, once the loop has finished, it returns Unit. For gives a fresh binding each turn because each loop, it takes the current environment and does the binding, assigning the output of that binding as a new environment. That new environment is only scoped for that iteration for the Rust for loop, so on the next iteration, a new environment will be created. When evaluating the expression inside the for loop, I use the ? operator, which will return out of the Rust for loop early if it spots an `Err` value. 

**In if 3 { 1 } else { 2 } the condition is not a Bool, and the program is stuck. Walk through how your Expr::If arm produces that, and contrast it with if false { 9 }, which is not stuck. Which branches does your arm evaluate in each case, and what does an else-less if return when its condition is false?**
The Expr::If evaluates the condition first (the 3 in this case), and checks if that is a bool using a match statement. Therefore, if it is an int, it will output a `TypeError`. `if false { 9 }` isn't stuck because the condition is a bool, which matches the premise of the evaluation. In the first case, the arm evaluates neither the `{1}` nor the `{2}` because the condition wasn't a bool. In the second case, it doesn't eval the `{9}` becasue the condition is false. Instead it checks for a right side, and because there is none, it returns `Unit`. An else-less if retuns a `Unit` if its condition is false.

## How you implemented it
**Walk through one of your loop arms (Expr::While or Expr::For): how you destructure it, how you re-evaluate the condition or iterate the list, where you take the body's value with ? and drop it, and how you produce Value::Unit. Then describe the two new cases you added inside Expr::Unary for UnOp::Ref and UnOp::Deref. Naming the provided tools you used (self.eval_expr, self.store.alloc/read/write, env.extend, RuntimeError, Ty::reference, .into()/?) is encouraged.**
For `Expr::While`, I start by checking that the condition is a bool. If so, I start a Rust while loop based on that condition, if not, I return a type error. After entering the loop (based on the condition), I run the body expression of the while loop, then re-evaluate the condition of the while loop (using `self.eval_expr`) and reassign the boolean that the loop is running on to the result of the re-evaluation. I also double check that the condition is still a bool just in case; if it isn't, I return a TypeError. I put `?` after my body evaluation to push an Err out if one arises. I produce Value::Unit simply by outputting that after the while loop. For `UnOp::Ref`, I call `store.alloc` on the value I get from the expression. For `UnOp::Deref`, I check that the expression evals to a `TyKind::Ref`. If not, I return a `TypeError`. If it is, I call `store.read` on that location and output the result of that.
Provided tools I used:
- self.eval_expr
- self.store.alloc
- self.store.read
- self.store.write
- env.extend
- RuntimeError::TypeError

**Point out anything that was tricky, a bug you fixed, or a design choice you made — and if you used an assistant, say what you had it do and how you checked its work.**
One thing that was tricky was figuring out how to start a Rust while loop with a boolean and then update that same boolean every loop of the while loop. It required re-evaluating the expression and double checking it was a Bool. I did not use an assistant.
