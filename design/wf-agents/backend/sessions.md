- 一条线的所有 session 共有的内容，放进这条线的新 session 开始时所用的 system prompt，排在一切因 session 而异的内容之前
  (wf-agents.md, "发给 agent 的内容按相同的范围由广到窄排列")。

- 等待中的 session，在最后一次调用之后 55 分钟、缓存放掉它之前，用一次 ping 保温；按模型的价格，
  ping 的次数不超过这段等待所值，因为一次 ping 花费很少，实测约 $0.03，而恢复一个已被缓存放掉的 session 非常昂贵。

- 在做 fork 之前，先用一次 ping 预热架构师的 base，除非它最后一次调用就是 55 分钟以内的 ping；架构师有工作在运行期间，
  base 也保温，因为 fork 从缓存读取 base，只能读到缓存仍保留的那次调用为止。实测中，一个没命中的 fork 把 base
  的 107K token 中约 73K 又写了一遍，多花了 $0.57，而一次 ping 才 $0.03，之后的每个 fork 也都一样。
