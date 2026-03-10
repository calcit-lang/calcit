
{} (:entries nil) (:package |app)
  :configs $ {} (:compact-output? true) (:extension |.cljs) (:init-fn |app.main/main!) (:local-ui? false) (:output |src) (:port 6001) (:reload-fn |app.main/reload!) (:version |0.0.1)
    :modules $ []
  :files $ {}
    |app.lib $ %{} :FileEntry
      :defs $ {}
        |f2 $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618661020393) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618661020393) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1618661020393) (:by |u0) (:text |f2)
              |r $ %{} :Expr (:at 1618661020393) (:by |u0)
                :data $ {}
              |v $ %{} :Expr (:at 1618661022794) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618661024070) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1618661026271) (:by |u0) (:text "|\"f2 in lib")
          :examples $ []
        |f3 $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618661052591) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618661052591) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1618661052591) (:by |u0) (:text |f3)
              |r $ %{} :Expr (:at 1618661052591) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618661067908) (:by |u0) (:text |x)
              |v $ %{} :Expr (:at 1618661054823) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618661055379) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1618661061473) (:by |u0) (:text "|\"f3 in lib")
              |x $ %{} :Expr (:at 1618661070479) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618661071077) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1618661073107) (:by |u0) (:text "|\"v:")
                  |r $ %{} :Leaf (:at 1618661074709) (:by |u0) (:text |x)
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ %{} :Expr (:at 1618661017191) (:by |u0)
          :data $ {}
            |T $ %{} :Leaf (:at 1618661017191) (:by |u0) (:text |ns)
            |j $ %{} :Leaf (:at 1618661017191) (:by |u0) (:text |app.lib)
    |app.macro $ %{} :FileEntry
      :defs $ {}
        |add-by-1 $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618740276250) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618740281235) (:by |u0) (:text |defmacro)
              |j $ %{} :Leaf (:at 1618740276250) (:by |u0) (:text |add-by-1)
              |r $ %{} :Expr (:at 1618740276250) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618740282976) (:by |u0) (:text |x)
              |v $ %{} :Expr (:at 1618740303995) (:by |u0)
                :data $ {}
                  |D $ %{} :Leaf (:at 1618740308945) (:by |u0) (:text |quasiquote)
                  |T $ %{} :Expr (:at 1618740285475) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1618740286902) (:by |u0) (:text |&+)
                      |j $ %{} :Leaf (:at 1618740317157) (:by |u0) (:text |~x)
                      |r $ %{} :Leaf (:at 1618740287700) (:by |u0) (:text |1)
          :examples $ []
        |add-by-2 $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618740293087) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618740296031) (:by |u0) (:text |defmacro)
              |j $ %{} :Leaf (:at 1618740293087) (:by |u0) (:text |add-by-2)
              |r $ %{} :Expr (:at 1618740293087) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618740299129) (:by |u0) (:text |x)
              |v $ %{} :Expr (:at 1618740300016) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618740325280) (:by |u0) (:text |quasiquote)
                  |j $ %{} :Expr (:at 1618740327115) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1618740331009) (:by |u0) (:text |&+)
                      |j $ %{} :Leaf (:at 1618740354540) (:by |u0) (:text |2)
                      |r $ %{} :Expr (:at 1618740340237) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1618740343769) (:by |u0) (:text |add-by-1)
                          |j $ %{} :Leaf (:at 1618740351578) (:by |u0) (:text |~x)
          :examples $ []
        |add-num $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1773136412558) (:by |sync)
            :data $ {}
              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |defmacro)
              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |add-num)
              |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |a)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |b)
              |Z $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |quasiquote)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&let)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                      |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&+)
                          |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |~)
                              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |a)
                          |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |~)
                              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |b)
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ %{} :Expr (:at 1618663277036) (:by |u0)
          :data $ {}
            |T $ %{} :Leaf (:at 1618663277036) (:by |u0) (:text |ns)
            |j $ %{} :Leaf (:at 1618663277036) (:by |u0) (:text |app.macro)
    |app.main $ %{} :FileEntry
      :defs $ {}
        |add-more $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618730350902) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618730354052) (:by |u0) (:text |defmacro)
              |j $ %{} :Leaf (:at 1618730350902) (:by |u0) (:text |add-more)
              |r $ %{} :Expr (:at 1618730350902) (:by |u0)
                :data $ {}
                  |D $ %{} :Leaf (:at 1618730403604) (:by |u0) (:text |acc)
                  |T $ %{} :Leaf (:at 1618730358202) (:by |u0) (:text |x)
                  |j $ %{} :Leaf (:at 1618730359828) (:by |u0) (:text |times)
              |v $ %{} :Expr (:at 1618730361081) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618730362447) (:by |u0) (:text |if)
                  |j $ %{} :Expr (:at 1618730365650) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1618730370296) (:by |u0) (:text |&<)
                      |b $ %{} :Leaf (:at 1618730372435) (:by |u0) (:text |times)
                      |j $ %{} :Leaf (:at 1618730539709) (:by |u0) (:text |1)
                  |r $ %{} :Leaf (:at 1618730533225) (:by |u0) (:text |acc)
                  |v $ %{} :Expr (:at 1618730378436) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1618730381681) (:by |u0) (:text |recur)
                      |j $ %{} :Expr (:at 1618730466064) (:by |u0)
                        :data $ {}
                          |D $ %{} :Leaf (:at 1618730500531) (:by |u0) (:text |quasiquote)
                          |T $ %{} :Expr (:at 1618730386375) (:by |u0)
                            :data $ {}
                              |D $ %{} :Leaf (:at 1618730388781) (:by |u0) (:text |&+)
                              |T $ %{} :Expr (:at 1618730485628) (:by |u0)
                                :data $ {}
                                  |D $ %{} :Leaf (:at 1618730486770) (:by |u0) (:text |~)
                                  |T $ %{} :Leaf (:at 1618730383299) (:by |u0) (:text |x)
                              |j $ %{} :Expr (:at 1618730488250) (:by |u0)
                                :data $ {}
                                  |D $ %{} :Leaf (:at 1618730489428) (:by |u0) (:text |~)
                                  |T $ %{} :Leaf (:at 1618730412605) (:by |u0) (:text |acc)
                      |n $ %{} :Leaf (:at 1618730516278) (:by |u0) (:text |x)
                      |r $ %{} :Expr (:at 1618730434451) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1618730435581) (:by |u0) (:text |&-)
                          |j $ %{} :Leaf (:at 1618730436881) (:by |u0) (:text |times)
                          |r $ %{} :Leaf (:at 1618730437157) (:by |u0) (:text |1)
          :examples $ []
        |call-3 $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618767957921) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618767957921) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1618767957921) (:by |u0) (:text |call-3)
              |r $ %{} :Expr (:at 1618767957921) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618767960551) (:by |u0) (:text |a)
                  |j $ %{} :Leaf (:at 1618767961787) (:by |u0) (:text |b)
                  |r $ %{} :Leaf (:at 1618767962162) (:by |u0) (:text |c)
              |v $ %{} :Expr (:at 1618767962704) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618767963282) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1618767965367) (:by |u0) (:text "|\"a is:")
                  |r $ %{} :Leaf (:at 1618767965784) (:by |u0) (:text |a)
              |x $ %{} :Expr (:at 1618767962704) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618767963282) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1618767969236) (:by |u0) (:text "|\"b is:")
                  |r $ %{} :Leaf (:at 1618767970341) (:by |u0) (:text |b)
              |y $ %{} :Expr (:at 1618767962704) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618767963282) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1618767977407) (:by |u0) (:text "|\"c is:")
                  |r $ %{} :Leaf (:at 1618767973639) (:by |u0) (:text |c)
          :examples $ []
        |call-macro $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618769676627) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618769678801) (:by |u0) (:text |defmacro)
              |j $ %{} :Leaf (:at 1618769676627) (:by |u0) (:text |call-macro)
              |r $ %{} :Expr (:at 1618769676627) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618769685522) (:by |u0) (:text |x0)
                  |j $ %{} :Leaf (:at 1618769686283) (:by |u0) (:text |&)
                  |r $ %{} :Leaf (:at 1618769686616) (:by |u0) (:text |xs)
              |v $ %{} :Expr (:at 1618769687244) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618769697898) (:by |u0) (:text |quasiquote)
                  |j $ %{} :Expr (:at 1618769717127) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1618769719548) (:by |u0) (:text |&{})
                      |j $ %{} :Leaf (:at 1618769720509) (:by |u0) (:text |:a)
                      |n $ %{} :Expr (:at 1618769729161) (:by |u0)
                        :data $ {}
                          |D $ %{} :Leaf (:at 1618769730971) (:by |u0) (:text |~)
                          |T $ %{} :Leaf (:at 1618769722734) (:by |u0) (:text |x0)
                      |r $ %{} :Leaf (:at 1618769723765) (:by |u0) (:text |:b)
                      |v $ %{} :Expr (:at 1618769809158) (:by |u0)
                        :data $ {}
                          |D $ %{} :Leaf (:at 1618769809634) (:by |u0) (:text |[])
                          |T $ %{} :Expr (:at 1618769725387) (:by |u0)
                            :data $ {}
                              |D $ %{} :Leaf (:at 1618769865395) (:by |u0) (:text |~@)
                              |T $ %{} :Leaf (:at 1618769725113) (:by |u0) (:text |xs)
          :examples $ []
        |call-many $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618769509051) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618769509051) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1618769509051) (:by |u0) (:text |call-many)
              |r $ %{} :Expr (:at 1618769509051) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618769511818) (:by |u0) (:text |x0)
                  |j $ %{} :Leaf (:at 1618769513121) (:by |u0) (:text |&)
                  |r $ %{} :Leaf (:at 1618769517543) (:by |u0) (:text |xs)
              |t $ %{} :Expr (:at 1618769532837) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618769533874) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1618769535535) (:by |u0) (:text "|\"many...")
              |v $ %{} :Expr (:at 1618769518829) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618769519471) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1618769522352) (:by |u0) (:text "|\"x0")
                  |r $ %{} :Leaf (:at 1618769523977) (:by |u0) (:text |x0)
              |x $ %{} :Expr (:at 1618769524533) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618769525175) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1618769525982) (:by |u0) (:text "|\"xs")
                  |r $ %{} :Leaf (:at 1618769526896) (:by |u0) (:text |xs)
          :examples $ []
        |demos $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1773136412558) (:by |sync)
            :data $ {}
              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |defn)
              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |demos)
              |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
              |Z $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"demo")
              |b $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&+)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
                      |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
              |d $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"f1")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |f1)
              |f $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&{})
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |:a)
                      |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                      |Z $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |:b)
                      |b $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
              |h $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |#{})
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                      |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
                      |Z $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |3)
                      |b $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text ||four)
              |j $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |lib/f2)
              |l $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |f3)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"arg of 3")
              |n $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"quote:")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |quote)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&+)
                          |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                          |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
              |p $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"quo:")
                  |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |'demo)
                  |Z $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |quote)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |'demo)
              |r $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"eval:")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |eval)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |quote)
                          |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&+)
                              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                              |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
              |t $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |if)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |true)
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"true")
              |v $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |if)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |false)
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"true")
                  |Z $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"false")
              |x $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |if)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&+)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                      |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"3")
                  |Z $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"?")
              |y $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&let)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |a)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"a is:")
                      |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |a)
              |z $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&let)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"a is none")
              |zV $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&let)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |a)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&+)
                          |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |3)
                          |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |4)
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"a is:")
                      |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |a)
              |zX $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |rest)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |[])
                          |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                          |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
                          |Z $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |3)
                          |b $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |4)
              |zZ $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |type-of)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |[])
                          |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
              |zb $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"result:")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |foldl)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |[])
                          |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                          |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
                          |Z $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |3)
                          |b $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |4)
                      |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |0)
                      |Z $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |defn)
                          |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |f1)
                          |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |acc)
                              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |x)
                          |Z $ %{} :Expr (:at 1773136412558) (:by |sync)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"adding:")
                              |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |acc)
                              |Z $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |x)
                          |b $ %{} :Expr (:at 1773136412558) (:by |sync)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |&+)
                              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |acc)
                              |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |x)
              |zd $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"macro:")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |add-num)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                      |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
              |zf $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"sum:")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |rec-sum)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |0)
                      |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |[])
                          |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                          |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
                          |Z $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |3)
                          |b $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |4)
              |zh $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"expand-1:")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |macroexpand-1)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |quote)
                          |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |add-num)
                              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                              |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
              |zj $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"expand:")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |macroexpand)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |quote)
                          |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |add-num)
                              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                              |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
              |zl $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"expand:")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |format-to-lisp)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |macroexpand)
                          |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |quote)
                              |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                                :data $ {}
                                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |add-more)
                                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |0)
                                  |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |3)
                                  |Z $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |8)
              |zn $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"expand v:")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |add-more)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |0)
                      |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |3)
                      |Z $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |8)
              |zp $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "|\"call and call")
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |add-by-2)
                      |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |10)
              |zr $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |;)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |macroexpand)
                      |V $ %{} :Expr (:at 1773136412558) (:by |sync)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |assert=)
                          |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |1)
                          |X $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |2)
              |zt $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |test-args)
          :examples $ []
        |f1 $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1773136412558) (:by |sync)
            :data $ {}
              |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |defn)
              |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |f1)
              |X $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
              |Z $ %{} :Expr (:at 1773136412558) (:by |sync)
                :data $ {}
                  |T $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text |println)
                  |V $ %{} :Leaf (:at 1773136412558) (:by |sync) (:text "||Hello with leaf!")
          :examples $ []
        |fib $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1619930459257) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1619930459257) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1619930459257) (:by |u0) (:text |fib)
              |r $ %{} :Expr (:at 1619930459257) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1619930460888) (:by |u0) (:text |n)
              |v $ %{} :Expr (:at 1619930461450) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1619930461900) (:by |u0) (:text |if)
                  |j $ %{} :Expr (:at 1619930462153) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1619930465800) (:by |u0) (:text |<)
                      |j $ %{} :Leaf (:at 1619930466571) (:by |u0) (:text |n)
                      |r $ %{} :Leaf (:at 1619930467516) (:by |u0) (:text |2)
                  |p $ %{} :Leaf (:at 1619976301564) (:by |u0) (:text |1)
                  |v $ %{} :Expr (:at 1619930469154) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1619930469867) (:by |u0) (:text |+)
                      |j $ %{} :Expr (:at 1619930471373) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1619930473045) (:by |u0) (:text |fib)
                          |j $ %{} :Expr (:at 1619930473244) (:by |u0)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1619930475429) (:by |u0) (:text |-)
                              |j $ %{} :Leaf (:at 1619930476120) (:by |u0) (:text |n)
                              |r $ %{} :Leaf (:at 1619930476518) (:by |u0) (:text |1)
                      |r $ %{} :Expr (:at 1619930471373) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1619930473045) (:by |u0) (:text |fib)
                          |j $ %{} :Expr (:at 1619930473244) (:by |u0)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1619930475429) (:by |u0) (:text |-)
                              |j $ %{} :Leaf (:at 1619930476120) (:by |u0) (:text |n)
                              |r $ %{} :Leaf (:at 1619930481371) (:by |u0) (:text |2)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1619930570377) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1619930570377) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1619930570377) (:by |u0) (:text |main!)
              |r $ %{} :Expr (:at 1619930570377) (:by |u0)
                :data $ {}
              |v $ %{} :Expr (:at 1619930574797) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1619930577305) (:by |u0) (:text |demos)
              |y $ %{} :Expr (:at 1619930582609) (:by |u0)
                :data $ {}
                  |D $ %{} :Leaf (:at 1622292794753) (:by |u0) (:text |;)
                  |T $ %{} :Leaf (:at 1619930582609) (:by |u0) (:text |fib)
                  |j $ %{} :Leaf (:at 1619930582609) (:by |u0) (:text |10)
              |yT $ %{} :Expr (:at 1622292783688) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1622292787836) (:by |u0) (:text |try-method)
              |yj $ %{} :Expr (:at 1633872988484) (:by |u0)
                :data $ {}
                  |D $ %{} :Leaf (:at 1633873455342) (:by |u0) (:text |;)
                  |T $ %{} :Leaf (:at 1633872991931) (:by |u0) (:text |show-data)
          :examples $ []
        |rec-sum $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618723127970) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618723127970) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1618723127970) (:by |u0) (:text |rec-sum)
              |r $ %{} :Expr (:at 1618723127970) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618723129611) (:by |u0) (:text |acc)
                  |j $ %{} :Leaf (:at 1618723131566) (:by |u0) (:text |xs)
              |v $ %{} :Expr (:at 1618723135708) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618723136188) (:by |u0) (:text |if)
                  |j $ %{} :Expr (:at 1618723136714) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1618723138019) (:by |u0) (:text |empty?)
                      |j $ %{} :Leaf (:at 1618723146569) (:by |u0) (:text |xs)
                  |r $ %{} :Leaf (:at 1618723147576) (:by |u0) (:text |acc)
                  |v $ %{} :Expr (:at 1618723147929) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1618723151992) (:by |u0) (:text |recur)
                      |j $ %{} :Expr (:at 1618723153359) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1618723158533) (:by |u0) (:text |&+)
                          |j $ %{} :Leaf (:at 1618723159204) (:by |u0) (:text |acc)
                          |r $ %{} :Expr (:at 1618723160405) (:by |u0)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1618723268153) (:by |u0) (:text |nth)
                              |j $ %{} :Leaf (:at 1618723162178) (:by |u0) (:text |xs)
                              |r $ %{} :Leaf (:at 1618723268981) (:by |u0) (:text |0)
                      |r $ %{} :Expr (:at 1618723164698) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1618723165126) (:by |u0) (:text |rest)
                          |j $ %{} :Leaf (:at 1618723165879) (:by |u0) (:text |xs)
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1619207810174) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1619207810174) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1619207810174) (:by |u0) (:text |reload!)
              |r $ %{} :Expr (:at 1619207810174) (:by |u0)
                :data $ {}
              |v $ %{} :Expr (:at 1619766026889) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1619766027788) (:by |u0) (:text |println)
                  |j $ %{} :Leaf (:at 1619766033570) (:by |u0) (:text "|\"reloaded 2")
              |x $ %{} :Expr (:at 1619930543193) (:by |u0)
                :data $ {}
                  |D $ %{} :Leaf (:at 1622292791514) (:by |u0) (:text |;)
                  |T $ %{} :Leaf (:at 1619930544016) (:by |u0) (:text |fib)
                  |j $ %{} :Leaf (:at 1619935071727) (:by |u0) (:text |40)
              |y $ %{} :Expr (:at 1622292799913) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1622292800206) (:by |u0) (:text |try-method)
          :examples $ []
        |show-data $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1633872992647) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1633872992647) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1633872992647) (:by |u0) (:text |show-data)
              |r $ %{} :Expr (:at 1633872992647) (:by |u0)
                :data $ {}
              |t $ %{} :Expr (:at 1633873024178) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1633873031232) (:by |u0) (:text |load-console-formatter!)
              |v $ %{} :Expr (:at 1633872993861) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1633872996602) (:by |u0) (:text |js/console.log)
                  |j $ %{} :Expr (:at 1633872997079) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1633873000863) (:by |u0) (:text |defrecord!)
                      |j $ %{} :Leaf (:at 1633873004188) (:by |u0) (:text |:Demo)
                      |r $ %{} :Expr (:at 1633873006952) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1633873004646) (:by |u0) (:text |:a)
                          |j $ %{} :Leaf (:at 1633873007810) (:by |u0) (:text |1)
                      |v $ %{} :Expr (:at 1633873008937) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1633873009838) (:by |u0) (:text |:b)
                          |j $ %{} :Expr (:at 1633873010851) (:by |u0)
                            :data $ {}
                              |T $ %{} :Leaf (:at 1633873011411) (:by |u0) (:text |{})
                              |j $ %{} :Expr (:at 1633873011697) (:by |u0)
                                :data $ {}
                                  |T $ %{} :Leaf (:at 1633873012008) (:by |u0) (:text |:a)
                                  |j $ %{} :Leaf (:at 1633873013762) (:by |u0) (:text |1)
          :examples $ []
        |test-args $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1618767933203) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1618767933203) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1618767933203) (:by |u0) (:text |test-args)
              |r $ %{} :Expr (:at 1618767933203) (:by |u0)
                :data $ {}
              |v $ %{} :Expr (:at 1618767936819) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618767946838) (:by |u0) (:text |call-3)
                  |b $ %{} :Leaf (:at 1618767951283) (:by |u0) (:text |&)
                  |j $ %{} :Expr (:at 1618767948145) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1618767948346) (:by |u0) (:text |[])
                      |j $ %{} :Leaf (:at 1618767949355) (:by |u0) (:text |1)
                      |r $ %{} :Leaf (:at 1618767949593) (:by |u0) (:text |2)
                      |v $ %{} :Leaf (:at 1618769480611) (:by |u0) (:text |3)
              |x $ %{} :Expr (:at 1618769504303) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618769507599) (:by |u0) (:text |call-many)
                  |j $ %{} :Leaf (:at 1618769530122) (:by |u0) (:text |1)
              |y $ %{} :Expr (:at 1618769504303) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618769507599) (:by |u0) (:text |call-many)
                  |b $ %{} :Leaf (:at 1618769543673) (:by |u0) (:text |1)
                  |j $ %{} :Leaf (:at 1618769540547) (:by |u0) (:text |2)
              |yT $ %{} :Expr (:at 1618769504303) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1618769507599) (:by |u0) (:text |call-many)
                  |j $ %{} :Leaf (:at 1618769545875) (:by |u0) (:text |1)
                  |r $ %{} :Leaf (:at 1618769546500) (:by |u0) (:text |2)
                  |v $ %{} :Leaf (:at 1618769546751) (:by |u0) (:text |3)
              |yj $ %{} :Expr (:at 1618769890713) (:by |u0)
                :data $ {}
                  |D $ %{} :Leaf (:at 1618769891472) (:by |u0) (:text |println)
                  |T $ %{} :Expr (:at 1618769885586) (:by |u0)
                    :data $ {}
                      |D $ %{} :Leaf (:at 1618769888788) (:by |u0) (:text |macroexpand)
                      |T $ %{} :Expr (:at 1618769673535) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1618769675192) (:by |u0) (:text |call-macro)
                          |j $ %{} :Leaf (:at 1618769762350) (:by |u0) (:text |11)
                          |r $ %{} :Leaf (:at 1618769837129) (:by |u0) (:text |12)
                          |v $ %{} :Leaf (:at 1618769849272) (:by |u0) (:text |13)
          :examples $ []
        |try-method $ %{} :CodeEntry (:doc |)
          :code $ %{} :Expr (:at 1622292801677) (:by |u0)
            :data $ {}
              |T $ %{} :Leaf (:at 1622292802864) (:by |u0) (:text |defn)
              |j $ %{} :Leaf (:at 1622292801677) (:by |u0) (:text |try-method)
              |r $ %{} :Expr (:at 1622292801677) (:by |u0)
                :data $ {}
              |v $ %{} :Expr (:at 1622292803720) (:by |u0)
                :data $ {}
                  |T $ %{} :Leaf (:at 1622292805545) (:by |u0) (:text |println)
                  |j $ %{} :Expr (:at 1622292805914) (:by |u0)
                    :data $ {}
                      |T $ %{} :Leaf (:at 1622292806869) (:by |u0) (:text |.count)
                      |j $ %{} :Expr (:at 1622292809130) (:by |u0)
                        :data $ {}
                          |T $ %{} :Leaf (:at 1622292811398) (:by |u0) (:text |range)
                          |j $ %{} :Leaf (:at 1622292816464) (:by |u0) (:text |11)
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ %{} :Expr (:at 1618539507433) (:by |u0)
          :data $ {}
            |T $ %{} :Leaf (:at 1618539507433) (:by |u0) (:text |ns)
            |j $ %{} :Leaf (:at 1618539507433) (:by |u0) (:text |app.main)
            |r $ %{} :Expr (:at 1618661030124) (:by |u0)
              :data $ {}
                |T $ %{} :Leaf (:at 1618661030826) (:by |u0) (:text |:require)
                |j $ %{} :Expr (:at 1618661031081) (:by |u0)
                  :data $ {}
                    |T $ %{} :Leaf (:at 1618661035015) (:by |u0) (:text |app.lib)
                    |j $ %{} :Leaf (:at 1618661039398) (:by |u0) (:text |:as)
                    |r $ %{} :Leaf (:at 1618661040510) (:by |u0) (:text |lib)
                |r $ %{} :Expr (:at 1618661042947) (:by |u0)
                  :data $ {}
                    |T $ %{} :Leaf (:at 1618661044709) (:by |u0) (:text |app.lib)
                    |j $ %{} :Leaf (:at 1618661045794) (:by |u0) (:text |:refer)
                    |r $ %{} :Expr (:at 1618661046024) (:by |u0)
                      :data $ {}
                        |T $ %{} :Leaf (:at 1618661046210) (:by |u0) (:text |[])
                        |j $ %{} :Leaf (:at 1618661047074) (:by |u0) (:text |f3)
                |v $ %{} :Expr (:at 1618720195824) (:by |u0)
                  :data $ {}
                    |T $ %{} :Leaf (:at 1618720199292) (:by |u0) (:text |app.macro)
                    |j $ %{} :Leaf (:at 1618720200969) (:by |u0) (:text |:refer)
                    |r $ %{} :Expr (:at 1618720201238) (:by |u0)
                      :data $ {}
                        |T $ %{} :Leaf (:at 1618720201399) (:by |u0) (:text |[])
                        |j $ %{} :Leaf (:at 1618720203059) (:by |u0) (:text |add-num)
                        |r $ %{} :Leaf (:at 1618740371002) (:by |u0) (:text |add-by-2)
  :users $ {}
    |u0 $ {} (:avatar nil) (:id |u0) (:name |chen) (:nickname |chen) (:password |d41d8cd98f00b204e9800998ecf8427e) (:theme :star-trail)
