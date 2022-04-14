
{}
  :configs $ {} (:reload-fn |app.main/reload!) (:port 6001) (:output |src) (:compact-output? true) (:version |0.0.1)
    :modules $ []
    :init-fn |app.main/main!
    :local-ui? false
    :extension |.cljs
  :ir $ {} (:package |app)
    :files $ {}
      |app.lib $ {}
        :defs $ {}
          |f2 $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1618661020393) (:by |u0)
              |j $ {} (:text |f2) (:type :leaf) (:at 1618661020393) (:by |u0)
              |r $ {}
                :data $ {}
                :type :expr
                :at 1618661020393
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618661024070) (:by |u0)
                  |j $ {} (:text "|\"f2 in lib") (:type :leaf) (:at 1618661026271) (:by |u0)
                :type :expr
                :at 1618661022794
                :by |u0
            :type :expr
            :at 1618661020393
            :by |u0
          |f3 $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1618661052591) (:by |u0)
              |j $ {} (:text |f3) (:type :leaf) (:at 1618661052591) (:by |u0)
              |r $ {}
                :data $ {}
                  |T $ {} (:text |x) (:type :leaf) (:at 1618661067908) (:by |u0)
                :type :expr
                :at 1618661052591
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618661055379) (:by |u0)
                  |j $ {} (:text "|\"f3 in lib") (:type :leaf) (:at 1618661061473) (:by |u0)
                :type :expr
                :at 1618661054823
                :by |u0
              |x $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618661071077) (:by |u0)
                  |j $ {} (:text "|\"v:") (:type :leaf) (:at 1618661073107) (:by |u0)
                  |r $ {} (:text |x) (:type :leaf) (:at 1618661074709) (:by |u0)
                :type :expr
                :at 1618661070479
                :by |u0
            :type :expr
            :at 1618661052591
            :by |u0
        :proc $ {}
          :data $ {}
          :type :expr
          :at 1618661017191
          :by |u0
        :configs $ {}
        :ns $ {}
          :data $ {}
            |T $ {} (:text |ns) (:type :leaf) (:at 1618661017191) (:by |u0)
            |j $ {} (:text |app.lib) (:type :leaf) (:at 1618661017191) (:by |u0)
          :type :expr
          :at 1618661017191
          :by |u0
      |app.macro $ {}
        :defs $ {}
          |add-by-1 $ {}
            :data $ {}
              |T $ {} (:text |defmacro) (:type :leaf) (:at 1618740281235) (:by |u0)
              |j $ {} (:text |add-by-1) (:type :leaf) (:at 1618740276250) (:by |u0)
              |r $ {}
                :data $ {}
                  |T $ {} (:text |x) (:type :leaf) (:at 1618740282976) (:by |u0)
                :type :expr
                :at 1618740276250
                :by |u0
              |v $ {}
                :data $ {}
                  |D $ {} (:text |quasiquote) (:type :leaf) (:at 1618740308945) (:by |u0)
                  |T $ {}
                    :data $ {}
                      |T $ {} (:text |&+) (:type :leaf) (:at 1618740286902) (:by |u0)
                      |j $ {} (:text |~x) (:type :leaf) (:at 1618740317157) (:by |u0)
                      |r $ {} (:text |1) (:type :leaf) (:at 1618740287700) (:by |u0)
                    :type :expr
                    :at 1618740285475
                    :by |u0
                :type :expr
                :at 1618740303995
                :by |u0
            :type :expr
            :at 1618740276250
            :by |u0
          |add-by-2 $ {}
            :data $ {}
              |T $ {} (:text |defmacro) (:type :leaf) (:at 1618740296031) (:by |u0)
              |j $ {} (:text |add-by-2) (:type :leaf) (:at 1618740293087) (:by |u0)
              |r $ {}
                :data $ {}
                  |T $ {} (:text |x) (:type :leaf) (:at 1618740299129) (:by |u0)
                :type :expr
                :at 1618740293087
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |quasiquote) (:type :leaf) (:at 1618740325280) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |&+) (:type :leaf) (:at 1618740331009) (:by |u0)
                      |j $ {} (:text |2) (:type :leaf) (:at 1618740354540) (:by |u0)
                      |r $ {}
                        :data $ {}
                          |T $ {} (:text |add-by-1) (:type :leaf) (:at 1618740343769) (:by |u0)
                          |j $ {} (:text |~x) (:type :leaf) (:at 1618740351578) (:by |u0)
                        :type :expr
                        :at 1618740340237
                        :by |u0
                    :type :expr
                    :at 1618740327115
                    :by |u0
                :type :expr
                :at 1618740300016
                :by |u0
            :type :expr
            :at 1618740293087
            :by |u0
          |add-num $ {}
            :data $ {}
              |T $ {} (:text |defmacro) (:type :leaf) (:at 1618663289791) (:by |u0)
              |j $ {} (:text |add-num) (:type :leaf) (:at 1618663286974) (:by |u0)
              |r $ {}
                :data $ {}
                  |T $ {} (:text |a) (:type :leaf) (:at 1618663291903) (:by |u0)
                  |j $ {} (:text |b) (:type :leaf) (:at 1618663292537) (:by |u0)
                :type :expr
                :at 1618663286974
                :by |u0
              |v $ {}
                :data $ {}
                  |D $ {} (:text |quasiquote) (:type :leaf) (:at 1618663328933) (:by |u0)
                  |T $ {}
                    :data $ {}
                      |T $ {} (:text |&let) (:type :leaf) (:at 1618663307918) (:by |u0)
                      |j $ {} (:text |nil) (:type :leaf) (:at 1618663305807) (:by |u0)
                      |r $ {}
                        :data $ {}
                          |T $ {} (:text |&+) (:type :leaf) (:at 1618663314951) (:by |u0)
                          |j $ {}
                            :data $ {}
                              |D $ {} (:text |~) (:type :leaf) (:at 1618663333114) (:by |u0)
                              |T $ {} (:text |a) (:type :leaf) (:at 1618663316680) (:by |u0)
                            :type :expr
                            :at 1618663331895
                            :by |u0
                          |r $ {}
                            :data $ {}
                              |D $ {} (:text |~) (:type :leaf) (:at 1618663336609) (:by |u0)
                              |T $ {} (:text |b) (:type :leaf) (:at 1618663317648) (:by |u0)
                            :type :expr
                            :at 1618663335086
                            :by |u0
                        :type :expr
                        :at 1618663312809
                        :by |u0
                    :type :expr
                    :at 1618663294505
                    :by |u0
                :type :expr
                :at 1618663324823
                :by |u0
            :type :expr
            :at 1618663286974
            :by |u0
        :proc $ {}
          :data $ {}
          :type :expr
          :at 1618663277036
          :by |u0
        :configs $ {}
        :ns $ {}
          :data $ {}
            |T $ {} (:text |ns) (:type :leaf) (:at 1618663277036) (:by |u0)
            |j $ {} (:text |app.macro) (:type :leaf) (:at 1618663277036) (:by |u0)
          :type :expr
          :at 1618663277036
          :by |u0
      |app.main $ {}
        :defs $ {}
          |call-macro $ {}
            :data $ {}
              |T $ {} (:text |defmacro) (:type :leaf) (:at 1618769678801) (:by |u0)
              |j $ {} (:text |call-macro) (:type :leaf) (:at 1618769676627) (:by |u0)
              |r $ {}
                :data $ {}
                  |T $ {} (:text |x0) (:type :leaf) (:at 1618769685522) (:by |u0)
                  |j $ {} (:text |&) (:type :leaf) (:at 1618769686283) (:by |u0)
                  |r $ {} (:text |xs) (:type :leaf) (:at 1618769686616) (:by |u0)
                :type :expr
                :at 1618769676627
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |quasiquote) (:type :leaf) (:at 1618769697898) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |&{}) (:type :leaf) (:at 1618769719548) (:by |u0)
                      |j $ {} (:text |:a) (:type :leaf) (:at 1618769720509) (:by |u0)
                      |n $ {}
                        :data $ {}
                          |D $ {} (:text |~) (:type :leaf) (:at 1618769730971) (:by |u0)
                          |T $ {} (:text |x0) (:type :leaf) (:at 1618769722734) (:by |u0)
                        :type :expr
                        :at 1618769729161
                        :by |u0
                      |r $ {} (:text |:b) (:type :leaf) (:at 1618769723765) (:by |u0)
                      |v $ {}
                        :data $ {}
                          |D $ {} (:text |[]) (:type :leaf) (:at 1618769809634) (:by |u0)
                          |T $ {}
                            :data $ {}
                              |D $ {} (:text |~@) (:type :leaf) (:at 1618769865395) (:by |u0)
                              |T $ {} (:text |xs) (:type :leaf) (:at 1618769725113) (:by |u0)
                            :type :expr
                            :at 1618769725387
                            :by |u0
                        :type :expr
                        :at 1618769809158
                        :by |u0
                    :type :expr
                    :at 1618769717127
                    :by |u0
                :type :expr
                :at 1618769687244
                :by |u0
            :type :expr
            :at 1618769676627
            :by |u0
          |show-data $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1633872992647) (:by |u0)
              |j $ {} (:text |show-data) (:type :leaf) (:at 1633872992647) (:by |u0)
              |r $ {}
                :data $ {}
                :type :expr
                :at 1633872992647
                :by |u0
              |t $ {}
                :data $ {}
                  |T $ {} (:text |load-console-formatter!) (:type :leaf) (:at 1633873031232) (:by |u0)
                :type :expr
                :at 1633873024178
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |js/console.log) (:type :leaf) (:at 1633872996602) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |defrecord!) (:type :leaf) (:at 1633873000863) (:by |u0)
                      |j $ {} (:text |:Demo) (:type :leaf) (:at 1633873004188) (:by |u0)
                      |r $ {}
                        :data $ {}
                          |T $ {} (:text |:a) (:type :leaf) (:at 1633873004646) (:by |u0)
                          |j $ {} (:text |1) (:type :leaf) (:at 1633873007810) (:by |u0)
                        :type :expr
                        :at 1633873006952
                        :by |u0
                      |v $ {}
                        :data $ {}
                          |T $ {} (:text |:b) (:type :leaf) (:at 1633873009838) (:by |u0)
                          |j $ {}
                            :data $ {}
                              |T $ {} (:text |{}) (:type :leaf) (:at 1633873011411) (:by |u0)
                              |j $ {}
                                :data $ {}
                                  |T $ {} (:text |:a) (:type :leaf) (:at 1633873012008) (:by |u0)
                                  |j $ {} (:text |1) (:type :leaf) (:at 1633873013762) (:by |u0)
                                :type :expr
                                :at 1633873011697
                                :by |u0
                            :type :expr
                            :at 1633873010851
                            :by |u0
                        :type :expr
                        :at 1633873008937
                        :by |u0
                    :type :expr
                    :at 1633872997079
                    :by |u0
                :type :expr
                :at 1633872993861
                :by |u0
            :type :expr
            :at 1633872992647
            :by |u0
          |fib $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1619930459257) (:by |u0)
              |j $ {} (:text |fib) (:type :leaf) (:at 1619930459257) (:by |u0)
              |r $ {}
                :data $ {}
                  |T $ {} (:text |n) (:type :leaf) (:at 1619930460888) (:by |u0)
                :type :expr
                :at 1619930459257
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |if) (:type :leaf) (:at 1619930461900) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |<) (:type :leaf) (:at 1619930465800) (:by |u0)
                      |j $ {} (:text |n) (:type :leaf) (:at 1619930466571) (:by |u0)
                      |r $ {} (:text |2) (:type :leaf) (:at 1619930467516) (:by |u0)
                    :type :expr
                    :at 1619930462153
                    :by |u0
                  |p $ {} (:text |1) (:type :leaf) (:at 1619976301564) (:by |u0)
                  |v $ {}
                    :data $ {}
                      |T $ {} (:text |+) (:type :leaf) (:at 1619930469867) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |fib) (:type :leaf) (:at 1619930473045) (:by |u0)
                          |j $ {}
                            :data $ {}
                              |T $ {} (:text |-) (:type :leaf) (:at 1619930475429) (:by |u0)
                              |j $ {} (:text |n) (:type :leaf) (:at 1619930476120) (:by |u0)
                              |r $ {} (:text |1) (:type :leaf) (:at 1619930476518) (:by |u0)
                            :type :expr
                            :at 1619930473244
                            :by |u0
                        :type :expr
                        :at 1619930471373
                        :by |u0
                      |r $ {}
                        :data $ {}
                          |T $ {} (:text |fib) (:type :leaf) (:at 1619930473045) (:by |u0)
                          |j $ {}
                            :data $ {}
                              |T $ {} (:text |-) (:type :leaf) (:at 1619930475429) (:by |u0)
                              |j $ {} (:text |n) (:type :leaf) (:at 1619930476120) (:by |u0)
                              |r $ {} (:text |2) (:type :leaf) (:at 1619930481371) (:by |u0)
                            :type :expr
                            :at 1619930473244
                            :by |u0
                        :type :expr
                        :at 1619930471373
                        :by |u0
                    :type :expr
                    :at 1619930469154
                    :by |u0
                :type :expr
                :at 1619930461450
                :by |u0
            :type :expr
            :at 1619930459257
            :by |u0
          |add-more $ {}
            :data $ {}
              |T $ {} (:text |defmacro) (:type :leaf) (:at 1618730354052) (:by |u0)
              |j $ {} (:text |add-more) (:type :leaf) (:at 1618730350902) (:by |u0)
              |r $ {}
                :data $ {}
                  |D $ {} (:text |acc) (:type :leaf) (:at 1618730403604) (:by |u0)
                  |T $ {} (:text |x) (:type :leaf) (:at 1618730358202) (:by |u0)
                  |j $ {} (:text |times) (:type :leaf) (:at 1618730359828) (:by |u0)
                :type :expr
                :at 1618730350902
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |if) (:type :leaf) (:at 1618730362447) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |&<) (:type :leaf) (:at 1618730370296) (:by |u0)
                      |b $ {} (:text |times) (:type :leaf) (:at 1618730372435) (:by |u0)
                      |j $ {} (:text |1) (:type :leaf) (:at 1618730539709) (:by |u0)
                    :type :expr
                    :at 1618730365650
                    :by |u0
                  |r $ {} (:text |acc) (:type :leaf) (:at 1618730533225) (:by |u0)
                  |v $ {}
                    :data $ {}
                      |T $ {} (:text |recur) (:type :leaf) (:at 1618730381681) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |D $ {} (:text |quasiquote) (:type :leaf) (:at 1618730500531) (:by |u0)
                          |T $ {}
                            :data $ {}
                              |D $ {} (:text |&+) (:type :leaf) (:at 1618730388781) (:by |u0)
                              |T $ {}
                                :data $ {}
                                  |D $ {} (:text |~) (:type :leaf) (:at 1618730486770) (:by |u0)
                                  |T $ {} (:text |x) (:type :leaf) (:at 1618730383299) (:by |u0)
                                :type :expr
                                :at 1618730485628
                                :by |u0
                              |j $ {}
                                :data $ {}
                                  |D $ {} (:text |~) (:type :leaf) (:at 1618730489428) (:by |u0)
                                  |T $ {} (:text |acc) (:type :leaf) (:at 1618730412605) (:by |u0)
                                :type :expr
                                :at 1618730488250
                                :by |u0
                            :type :expr
                            :at 1618730386375
                            :by |u0
                        :type :expr
                        :at 1618730466064
                        :by |u0
                      |n $ {} (:text |x) (:type :leaf) (:at 1618730516278) (:by |u0)
                      |r $ {}
                        :data $ {}
                          |T $ {} (:text |&-) (:type :leaf) (:at 1618730435581) (:by |u0)
                          |j $ {} (:text |times) (:type :leaf) (:at 1618730436881) (:by |u0)
                          |r $ {} (:text |1) (:type :leaf) (:at 1618730437157) (:by |u0)
                        :type :expr
                        :at 1618730434451
                        :by |u0
                    :type :expr
                    :at 1618730378436
                    :by |u0
                :type :expr
                :at 1618730361081
                :by |u0
            :type :expr
            :at 1618730350902
            :by |u0
          |test-args $ {}
            :data $ {}
              |yT $ {}
                :data $ {}
                  |T $ {} (:text |call-many) (:type :leaf) (:at 1618769507599) (:by |u0)
                  |j $ {} (:text |1) (:type :leaf) (:at 1618769545875) (:by |u0)
                  |r $ {} (:text |2) (:type :leaf) (:at 1618769546500) (:by |u0)
                  |v $ {} (:text |3) (:type :leaf) (:at 1618769546751) (:by |u0)
                :type :expr
                :at 1618769504303
                :by |u0
              |yj $ {}
                :data $ {}
                  |D $ {} (:text |println) (:type :leaf) (:at 1618769891472) (:by |u0)
                  |T $ {}
                    :data $ {}
                      |D $ {} (:text |macroexpand) (:type :leaf) (:at 1618769888788) (:by |u0)
                      |T $ {}
                        :data $ {}
                          |T $ {} (:text |call-macro) (:type :leaf) (:at 1618769675192) (:by |u0)
                          |j $ {} (:text |11) (:type :leaf) (:at 1618769762350) (:by |u0)
                          |r $ {} (:text |12) (:type :leaf) (:at 1618769837129) (:by |u0)
                          |v $ {} (:text |13) (:type :leaf) (:at 1618769849272) (:by |u0)
                        :type :expr
                        :at 1618769673535
                        :by |u0
                    :type :expr
                    :at 1618769885586
                    :by |u0
                :type :expr
                :at 1618769890713
                :by |u0
              |T $ {} (:text |defn) (:type :leaf) (:at 1618767933203) (:by |u0)
              |j $ {} (:text |test-args) (:type :leaf) (:at 1618767933203) (:by |u0)
              |r $ {}
                :data $ {}
                :type :expr
                :at 1618767933203
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |call-3) (:type :leaf) (:at 1618767946838) (:by |u0)
                  |b $ {} (:text |&) (:type :leaf) (:at 1618767951283) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |[]) (:type :leaf) (:at 1618767948346) (:by |u0)
                      |j $ {} (:text |1) (:type :leaf) (:at 1618767949355) (:by |u0)
                      |r $ {} (:text |2) (:type :leaf) (:at 1618767949593) (:by |u0)
                      |v $ {} (:text |3) (:type :leaf) (:at 1618769480611) (:by |u0)
                    :type :expr
                    :at 1618767948145
                    :by |u0
                :type :expr
                :at 1618767936819
                :by |u0
              |x $ {}
                :data $ {}
                  |T $ {} (:text |call-many) (:type :leaf) (:at 1618769507599) (:by |u0)
                  |j $ {} (:text |1) (:type :leaf) (:at 1618769530122) (:by |u0)
                :type :expr
                :at 1618769504303
                :by |u0
              |y $ {}
                :data $ {}
                  |T $ {} (:text |call-many) (:type :leaf) (:at 1618769507599) (:by |u0)
                  |b $ {} (:text |1) (:type :leaf) (:at 1618769543673) (:by |u0)
                  |j $ {} (:text |2) (:type :leaf) (:at 1618769540547) (:by |u0)
                :type :expr
                :at 1618769504303
                :by |u0
            :type :expr
            :at 1618767933203
            :by |u0
          |call-many $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1618769509051) (:by |u0)
              |j $ {} (:text |call-many) (:type :leaf) (:at 1618769509051) (:by |u0)
              |r $ {}
                :data $ {}
                  |T $ {} (:text |x0) (:type :leaf) (:at 1618769511818) (:by |u0)
                  |j $ {} (:text |&) (:type :leaf) (:at 1618769513121) (:by |u0)
                  |r $ {} (:text |xs) (:type :leaf) (:at 1618769517543) (:by |u0)
                :type :expr
                :at 1618769509051
                :by |u0
              |t $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618769533874) (:by |u0)
                  |j $ {} (:text "|\"many...") (:type :leaf) (:at 1618769535535) (:by |u0)
                :type :expr
                :at 1618769532837
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618769519471) (:by |u0)
                  |j $ {} (:text "|\"x0") (:type :leaf) (:at 1618769522352) (:by |u0)
                  |r $ {} (:text |x0) (:type :leaf) (:at 1618769523977) (:by |u0)
                :type :expr
                :at 1618769518829
                :by |u0
              |x $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618769525175) (:by |u0)
                  |j $ {} (:text "|\"xs") (:type :leaf) (:at 1618769525982) (:by |u0)
                  |r $ {} (:text |xs) (:type :leaf) (:at 1618769526896) (:by |u0)
                :type :expr
                :at 1618769524533
                :by |u0
            :type :expr
            :at 1618769509051
            :by |u0
          |demos $ {}
            :data $ {}
              |yyT $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618664966725) (:by |u0)
                  |j $ {} (:text "|\"quote:") (:type :leaf) (:at 1618664980683) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |quote) (:type :leaf) (:at 1618664969526) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |&+) (:type :leaf) (:at 1618665001007) (:by |u0)
                          |j $ {} (:text |1) (:type :leaf) (:at 1618664970588) (:by |u0)
                          |r $ {} (:text |2) (:type :leaf) (:at 1618664970840) (:by |u0)
                        :type :expr
                        :at 1618664969796
                        :by |u0
                    :type :expr
                    :at 1618664968766
                    :by |u0
                :type :expr
                :at 1618664966181
                :by |u0
              |yyb $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618665182898) (:by |u0)
                  |j $ {} (:text "|\"quo:") (:type :leaf) (:at 1618665185901) (:by |u0)
                  |r $ {} (:text |'demo) (:type :leaf) (:at 1618665190172) (:by |u0)
                  |v $ {}
                    :data $ {}
                      |T $ {} (:text |quote) (:type :leaf) (:at 1618665202393) (:by |u0)
                      |j $ {} (:text |'demo) (:type :leaf) (:at 1618665203149) (:by |u0)
                    :type :expr
                    :at 1618665201691
                    :by |u0
                :type :expr
                :at 1618665182369
                :by |u0
              |yyj $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618664972897) (:by |u0)
                  |j $ {} (:text "|\"eval:") (:type :leaf) (:at 1618664978986) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |eval) (:type :leaf) (:at 1618664982687) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |quote) (:type :leaf) (:at 1618664984086) (:by |u0)
                          |j $ {}
                            :data $ {}
                              |T $ {} (:text |&+) (:type :leaf) (:at 1618664995431) (:by |u0)
                              |j $ {} (:text |1) (:type :leaf) (:at 1618664985011) (:by |u0)
                              |r $ {} (:text |2) (:type :leaf) (:at 1618664985257) (:by |u0)
                            :type :expr
                            :at 1618664984358
                            :by |u0
                        :type :expr
                        :at 1618664983058
                        :by |u0
                    :type :expr
                    :at 1618664981960
                    :by |u0
                :type :expr
                :at 1618664972310
                :by |u0
              |yyr $ {}
                :data $ {}
                  |T $ {} (:text |if) (:type :leaf) (:at 1618673510809) (:by |u0)
                  |j $ {} (:text |true) (:type :leaf) (:at 1618673513600) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |println) (:type :leaf) (:at 1618673514609) (:by |u0)
                      |j $ {} (:text "|\"true") (:type :leaf) (:at 1618673517373) (:by |u0)
                    :type :expr
                    :at 1618673514067
                    :by |u0
                :type :expr
                :at 1618673510188
                :by |u0
              |yyv $ {}
                :data $ {}
                  |T $ {} (:text |if) (:type :leaf) (:at 1618673510809) (:by |u0)
                  |j $ {} (:text |false) (:type :leaf) (:at 1618673522034) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |println) (:type :leaf) (:at 1618673514609) (:by |u0)
                      |j $ {} (:text "|\"true") (:type :leaf) (:at 1618673517373) (:by |u0)
                    :type :expr
                    :at 1618673514067
                    :by |u0
                  |v $ {}
                    :data $ {}
                      |T $ {} (:text |println) (:type :leaf) (:at 1618673525729) (:by |u0)
                      |j $ {} (:text "|\"false") (:type :leaf) (:at 1618673526734) (:by |u0)
                    :type :expr
                    :at 1618673524977
                    :by |u0
                :type :expr
                :at 1618673510188
                :by |u0
              |yyx $ {}
                :data $ {}
                  |T $ {} (:text |if) (:type :leaf) (:at 1618673529821) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |&+) (:type :leaf) (:at 1618673534134) (:by |u0)
                      |j $ {} (:text |1) (:type :leaf) (:at 1618673534565) (:by |u0)
                      |r $ {} (:text |2) (:type :leaf) (:at 1618673534799) (:by |u0)
                    :type :expr
                    :at 1618673530125
                    :by |u0
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |println) (:type :leaf) (:at 1618673536109) (:by |u0)
                      |j $ {} (:text "|\"3") (:type :leaf) (:at 1618673538376) (:by |u0)
                    :type :expr
                    :at 1618673537272
                    :by |u0
                  |v $ {}
                    :data $ {}
                      |T $ {} (:text |println) (:type :leaf) (:at 1618673541276) (:by |u0)
                      |j $ {} (:text "|\"?") (:type :leaf) (:at 1618673542363) (:by |u0)
                    :type :expr
                    :at 1618673540682
                    :by |u0
                :type :expr
                :at 1618673529205
                :by |u0
              |yyy $ {}
                :data $ {}
                  |T $ {} (:text |&let) (:type :leaf) (:at 1618674587642) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |a) (:type :leaf) (:at 1618674589371) (:by |u0)
                      |j $ {} (:text |1) (:type :leaf) (:at 1618674589618) (:by |u0)
                    :type :expr
                    :at 1618674588361
                    :by |u0
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |println) (:type :leaf) (:at 1618674592232) (:by |u0)
                      |j $ {} (:text "|\"a is:") (:type :leaf) (:at 1618674596559) (:by |u0)
                      |r $ {} (:text |a) (:type :leaf) (:at 1618674595408) (:by |u0)
                    :type :expr
                    :at 1618674591714
                    :by |u0
                :type :expr
                :at 1618674585688
                :by |u0
              |yyyT $ {}
                :data $ {}
                  |T $ {} (:text |&let) (:type :leaf) (:at 1618674587642) (:by |u0)
                  |f $ {} (:text |nil) (:type :leaf) (:at 1618674603307) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |println) (:type :leaf) (:at 1618674592232) (:by |u0)
                      |j $ {} (:text "|\"a is none") (:type :leaf) (:at 1618674610267) (:by |u0)
                    :type :expr
                    :at 1618674591714
                    :by |u0
                :type :expr
                :at 1618674585688
                :by |u0
              |yyyj $ {}
                :data $ {}
                  |T $ {} (:text |&let) (:type :leaf) (:at 1618674612756) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |a) (:type :leaf) (:at 1618674613637) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |&+) (:type :leaf) (:at 1618674617692) (:by |u0)
                          |j $ {} (:text |3) (:type :leaf) (:at 1618674618272) (:by |u0)
                          |r $ {} (:text |4) (:type :leaf) (:at 1618674618576) (:by |u0)
                        :type :expr
                        :at 1618674615215
                        :by |u0
                    :type :expr
                    :at 1618674613267
                    :by |u0
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |println) (:type :leaf) (:at 1618674621967) (:by |u0)
                      |j $ {} (:text "|\"a is:") (:type :leaf) (:at 1618674624057) (:by |u0)
                      |r $ {} (:text |a) (:type :leaf) (:at 1618674624971) (:by |u0)
                    :type :expr
                    :at 1618674621227
                    :by |u0
                :type :expr
                :at 1618674611597
                :by |u0
              |yyyr $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618681701504) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |rest) (:type :leaf) (:at 1618681702755) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |[]) (:type :leaf) (:at 1618681704264) (:by |u0)
                          |j $ {} (:text |1) (:type :leaf) (:at 1618681704468) (:by |u0)
                          |r $ {} (:text |2) (:type :leaf) (:at 1618681704653) (:by |u0)
                          |v $ {} (:text |3) (:type :leaf) (:at 1618681705572) (:by |u0)
                          |x $ {} (:text |4) (:type :leaf) (:at 1618681705808) (:by |u0)
                        :type :expr
                        :at 1618681703369
                        :by |u0
                    :type :expr
                    :at 1618681701785
                    :by |u0
                :type :expr
                :at 1618681700994
                :by |u0
              |yyyv $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618682122607) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |type-of) (:type :leaf) (:at 1618682124422) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |[]) (:type :leaf) (:at 1618682124941) (:by |u0)
                          |j $ {} (:text |1) (:type :leaf) (:at 1618682127480) (:by |u0)
                        :type :expr
                        :at 1618682124681
                        :by |u0
                    :type :expr
                    :at 1618682123605
                    :by |u0
                :type :expr
                :at 1618682122124
                :by |u0
              |yyyx $ {}
                :data $ {}
                  |D $ {} (:text |println) (:type :leaf) (:at 1618682971333) (:by |u0)
                  |L $ {} (:text "|\"result:") (:type :leaf) (:at 1618682973563) (:by |u0)
                  |T $ {}
                    :data $ {}
                      |T $ {} (:text |foldl) (:type :leaf) (:at 1618682940605) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |[]) (:type :leaf) (:at 1618682942650) (:by |u0)
                          |j $ {} (:text |1) (:type :leaf) (:at 1618682944334) (:by |u0)
                          |r $ {} (:text |2) (:type :leaf) (:at 1618682944566) (:by |u0)
                          |v $ {} (:text |3) (:type :leaf) (:at 1618682944835) (:by |u0)
                          |x $ {} (:text |4) (:type :leaf) (:at 1618682945203) (:by |u0)
                        :type :expr
                        :at 1618682942439
                        :by |u0
                      |r $ {} (:text |0) (:type :leaf) (:at 1618682947341) (:by |u0)
                      |v $ {}
                        :data $ {}
                          |T $ {} (:text |defn) (:type :leaf) (:at 1618682953315) (:by |u0)
                          |j $ {} (:text |f1) (:type :leaf) (:at 1618682955242) (:by |u0)
                          |r $ {}
                            :data $ {}
                              |T $ {} (:text |acc) (:type :leaf) (:at 1618682958260) (:by |u0)
                              |j $ {} (:text |x) (:type :leaf) (:at 1618682958862) (:by |u0)
                            :type :expr
                            :at 1618682956170
                            :by |u0
                          |t $ {}
                            :data $ {}
                              |T $ {} (:text |println) (:type :leaf) (:at 1618682976544) (:by |u0)
                              |j $ {} (:text "|\"adding:") (:type :leaf) (:at 1618682979610) (:by |u0)
                              |n $ {} (:text |acc) (:type :leaf) (:at 1618683016109) (:by |u0)
                              |r $ {} (:text |x) (:type :leaf) (:at 1618682978465) (:by |u0)
                            :type :expr
                            :at 1618682975336
                            :by |u0
                          |v $ {}
                            :data $ {}
                              |T $ {} (:text |&+) (:type :leaf) (:at 1618682965361) (:by |u0)
                              |j $ {} (:text |acc) (:type :leaf) (:at 1618682962994) (:by |u0)
                              |r $ {} (:text |x) (:type :leaf) (:at 1618682964049) (:by |u0)
                            :type :expr
                            :at 1618682960354
                            :by |u0
                        :type :expr
                        :at 1618682949689
                        :by |u0
                    :type :expr
                    :at 1618682938708
                    :by |u0
                :type :expr
                :at 1618682969714
                :by |u0
              |yyyy $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618720206820) (:by |u0)
                  |j $ {} (:text "|\"macro:") (:type :leaf) (:at 1618720208707) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |add-num) (:type :leaf) (:at 1618720209139) (:by |u0)
                      |j $ {} (:text |1) (:type :leaf) (:at 1618720211273) (:by |u0)
                      |r $ {} (:text |2) (:type :leaf) (:at 1618720211522) (:by |u0)
                    :type :expr
                    :at 1618720210191
                    :by |u0
                :type :expr
                :at 1618720206313
                :by |u0
              |yyyyT $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618723114194) (:by |u0)
                  |j $ {} (:text "|\"sum:") (:type :leaf) (:at 1618723701346) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |rec-sum) (:type :leaf) (:at 1618723121717) (:by |u0)
                      |j $ {} (:text |0) (:type :leaf) (:at 1618723122699) (:by |u0)
                      |r $ {}
                        :data $ {}
                          |T $ {} (:text |[]) (:type :leaf) (:at 1618723123387) (:by |u0)
                          |j $ {} (:text |1) (:type :leaf) (:at 1618723124101) (:by |u0)
                          |r $ {} (:text |2) (:type :leaf) (:at 1618723124374) (:by |u0)
                          |v $ {} (:text |3) (:type :leaf) (:at 1618723124700) (:by |u0)
                          |x $ {} (:text |4) (:type :leaf) (:at 1618723125706) (:by |u0)
                        :type :expr
                        :at 1618723123028
                        :by |u0
                    :type :expr
                    :at 1618723116484
                    :by |u0
                :type :expr
                :at 1618723113290
                :by |u0
              |yyyyb $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618729369263) (:by |u0)
                  |j $ {} (:text "|\"expand-1:") (:type :leaf) (:at 1618729369263) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |macroexpand-1) (:type :leaf) (:at 1618729369263) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |quote) (:type :leaf) (:at 1618729369263) (:by |u0)
                          |j $ {}
                            :data $ {}
                              |T $ {} (:text |add-num) (:type :leaf) (:at 1618729369263) (:by |u0)
                              |j $ {} (:text |1) (:type :leaf) (:at 1618729369263) (:by |u0)
                              |r $ {} (:text |2) (:type :leaf) (:at 1618729369263) (:by |u0)
                            :type :expr
                            :at 1618729369263
                            :by |u0
                        :type :expr
                        :at 1618729369263
                        :by |u0
                    :type :expr
                    :at 1618729369263
                    :by |u0
                :type :expr
                :at 1618729369263
                :by |u0
              |yyyyj $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618728236844) (:by |u0)
                  |j $ {} (:text "|\"expand:") (:type :leaf) (:at 1618728240766) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |macroexpand) (:type :leaf) (:at 1618729257611) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |D $ {} (:text |quote) (:type :leaf) (:at 1618728293719) (:by |u0)
                          |T $ {}
                            :data $ {}
                              |T $ {} (:text |add-num) (:type :leaf) (:at 1618728250500) (:by |u0)
                              |j $ {} (:text |1) (:type :leaf) (:at 1618728250838) (:by |u0)
                              |r $ {} (:text |2) (:type :leaf) (:at 1618728251146) (:by |u0)
                            :type :expr
                            :at 1618728247075
                            :by |u0
                        :type :expr
                        :at 1618728292870
                        :by |u0
                    :type :expr
                    :at 1618728241448
                    :by |u0
                :type :expr
                :at 1618728236147
                :by |u0
              |yyyyr $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618728236844) (:by |u0)
                  |j $ {} (:text "|\"expand:") (:type :leaf) (:at 1618728240766) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |D $ {} (:text |format-to-lisp) (:type :leaf) (:at 1618769245430) (:by |u0)
                      |T $ {}
                        :data $ {}
                          |T $ {} (:text |macroexpand) (:type :leaf) (:at 1618729257611) (:by |u0)
                          |j $ {}
                            :data $ {}
                              |D $ {} (:text |quote) (:type :leaf) (:at 1618728293719) (:by |u0)
                              |T $ {}
                                :data $ {}
                                  |T $ {} (:text |add-more) (:type :leaf) (:at 1618730300485) (:by |u0)
                                  |b $ {} (:text |0) (:type :leaf) (:at 1618730406639) (:by |u0)
                                  |j $ {} (:text |3) (:type :leaf) (:at 1618730347804) (:by |u0)
                                  |r $ {} (:text |8) (:type :leaf) (:at 1618730348853) (:by |u0)
                                :type :expr
                                :at 1618728247075
                                :by |u0
                            :type :expr
                            :at 1618728292870
                            :by |u0
                        :type :expr
                        :at 1618728241448
                        :by |u0
                    :type :expr
                    :at 1618769244761
                    :by |u0
                :type :expr
                :at 1618728236147
                :by |u0
              |yyyyv $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618728236844) (:by |u0)
                  |j $ {} (:text "|\"expand v:") (:type :leaf) (:at 1618730586955) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |add-more) (:type :leaf) (:at 1618730585215) (:by |u0)
                      |j $ {} (:text |0) (:type :leaf) (:at 1618730585215) (:by |u0)
                      |r $ {} (:text |3) (:type :leaf) (:at 1618730585215) (:by |u0)
                      |v $ {} (:text |8) (:type :leaf) (:at 1618730585215) (:by |u0)
                    :type :expr
                    :at 1618730585215
                    :by |u0
                :type :expr
                :at 1618728236147
                :by |u0
              |yyyyx $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618740378663) (:by |u0)
                  |j $ {} (:text "|\"call and call") (:type :leaf) (:at 1618740385798) (:by |u0)
                  |r $ {}
                    :data $ {}
                      |T $ {} (:text |add-by-2) (:type :leaf) (:at 1618740386840) (:by |u0)
                      |j $ {} (:text |10) (:type :leaf) (:at 1618740388181) (:by |u0)
                    :type :expr
                    :at 1618740386339
                    :by |u0
                :type :expr
                :at 1618740378070
                :by |u0
              |yyyyy $ {}
                :data $ {}
                  |5 $ {} (:text |;) (:type :leaf) (:at 1618772534094) (:by |u0)
                  |D $ {} (:text |println) (:type :leaf) (:at 1618770030105) (:by |u0)
                  |T $ {}
                    :data $ {}
                      |D $ {} (:text |macroexpand) (:type :leaf) (:at 1618770034555) (:by |u0)
                      |T $ {}
                        :data $ {}
                          |T $ {} (:text |assert=) (:type :leaf) (:at 1618752133902) (:by |u0)
                          |j $ {} (:text |1) (:type :leaf) (:at 1618752134923) (:by |u0)
                          |r $ {} (:text |2) (:type :leaf) (:at 1618752135294) (:by |u0)
                        :type :expr
                        :at 1618752131764
                        :by |u0
                    :type :expr
                    :at 1618770031138
                    :by |u0
                :type :expr
                :at 1618770028090
                :by |u0
              |yT $ {}
                :data $ {}
                  |T $ {} (:text |print-values) (:type :leaf) (:at 1633952520593) (:by |u0)
                  |j $ {} (:text |1) (:type :leaf) (:at 1618659590535) (:by |u0)
                  |r $ {} (:text "|\"1") (:type :leaf) (:at 1618659591512) (:by |u0)
                  |v $ {} (:text |:a) (:type :leaf) (:at 1618659595541) (:by |u0)
                  |x $ {}
                    :data $ {}
                      |T $ {} (:text |[]) (:type :leaf) (:at 1618659596880) (:by |u0)
                      |j $ {} (:text |1) (:type :leaf) (:at 1618659597668) (:by |u0)
                      |r $ {} (:text |2) (:type :leaf) (:at 1618659597892) (:by |u0)
                    :type :expr
                    :at 1618659596691
                    :by |u0
                :type :expr
                :at 1618659585738
                :by |u0
              |yj $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618660537901) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |&{}) (:type :leaf) (:at 1618660568253) (:by |u0)
                      |j $ {} (:text |:a) (:type :leaf) (:at 1618660541656) (:by |u0)
                      |r $ {} (:text |1) (:type :leaf) (:at 1618660542971) (:by |u0)
                      |v $ {} (:text |:b) (:type :leaf) (:at 1618660543782) (:by |u0)
                      |x $ {} (:text |2) (:type :leaf) (:at 1618660544981) (:by |u0)
                    :type :expr
                    :at 1618660538186
                    :by |u0
                :type :expr
                :at 1618660536373
                :by |u0
              |yr $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618660963956) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |#{}) (:type :leaf) (:at 1618660965160) (:by |u0)
                      |j $ {} (:text |1) (:type :leaf) (:at 1618660965550) (:by |u0)
                      |r $ {} (:text |2) (:type :leaf) (:at 1618660965773) (:by |u0)
                      |v $ {} (:text |3) (:type :leaf) (:at 1618660966299) (:by |u0)
                      |x $ {} (:text ||four) (:type :leaf) (:at 1618660970012) (:by |u0)
                    :type :expr
                    :at 1618660964279
                    :by |u0
                :type :expr
                :at 1618660963223
                :by |u0
              |yx $ {}
                :data $ {}
                  |T $ {} (:text |lib/f2) (:type :leaf) (:at 1618661298818) (:by |u0)
                :type :expr
                :at 1618661082170
                :by |u0
              |yy $ {}
                :data $ {}
                  |T $ {} (:text |f3) (:type :leaf) (:at 1618661302264) (:by |u0)
                  |j $ {} (:text "|\"arg of 3") (:type :leaf) (:at 1618661308107) (:by |u0)
                :type :expr
                :at 1618661300982
                :by |u0
              |T $ {} (:text |defn) (:type :leaf) (:at 1618539520156) (:by |u0)
              |j $ {} (:text |demos) (:type :leaf) (:at 1619930563832) (:by |u0)
              |r $ {}
                :data $ {}
                :type :expr
                :at 1618539520156
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618539524965) (:by |u0)
                  |j $ {} (:text "|\"demo") (:type :leaf) (:at 1618539525898) (:by |u0)
                :type :expr
                :at 1618539523268
                :by |u0
              |x $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618646119371) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |&+) (:type :leaf) (:at 1618646122999) (:by |u0)
                      |j $ {} (:text |2) (:type :leaf) (:at 1618658555366) (:by |u0)
                      |r $ {} (:text |2) (:type :leaf) (:at 1618646121081) (:by |u0)
                    :type :expr
                    :at 1618646119955
                    :by |u0
                :type :expr
                :at 1618646117925
                :by |u0
              |y $ {}
                :data $ {}
                  |D $ {} (:text |println) (:type :leaf) (:at 1618658519944) (:by |u0)
                  |L $ {} (:text "|\"f1") (:type :leaf) (:at 1618658520784) (:by |u0)
                  |T $ {}
                    :data $ {}
                      |T $ {} (:text |f1) (:type :leaf) (:at 1618658495406) (:by |u0)
                    :type :expr
                    :at 1618658494170
                    :by |u0
                :type :expr
                :at 1618658517774
                :by |u0
              |yyyyyT $ {}
                :data $ {}
                  |T $ {} (:text |test-args) (:type :leaf) (:at 1618767932151) (:by |u0)
                :type :expr
                :at 1618767923138
                :by |u0
            :type :expr
            :at 1618539520156
            :by |u0
          |main! $ {}
            :data $ {}
              |yT $ {}
                :data $ {}
                  |T $ {} (:text |try-method) (:type :leaf) (:at 1622292787836) (:by |u0)
                :type :expr
                :at 1622292783688
                :by |u0
              |yj $ {}
                :data $ {}
                  |D $ {} (:text |;) (:type :leaf) (:at 1633873455342) (:by |u0)
                  |T $ {} (:text |show-data) (:type :leaf) (:at 1633872991931) (:by |u0)
                :type :expr
                :at 1633872988484
                :by |u0
              |T $ {} (:text |defn) (:type :leaf) (:at 1619930570377) (:by |u0)
              |j $ {} (:text |main!) (:type :leaf) (:at 1619930570377) (:by |u0)
              |r $ {}
                :data $ {}
                :type :expr
                :at 1619930570377
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |demos) (:type :leaf) (:at 1619930577305) (:by |u0)
                :type :expr
                :at 1619930574797
                :by |u0
              |y $ {}
                :data $ {}
                  |D $ {} (:text |;) (:type :leaf) (:at 1622292794753) (:by |u0)
                  |T $ {} (:text |fib) (:type :leaf) (:at 1619930582609) (:by |u0)
                  |j $ {} (:text |10) (:type :leaf) (:at 1619930582609) (:by |u0)
                :type :expr
                :at 1619930582609
                :by |u0
            :type :expr
            :at 1619930570377
            :by |u0
          |f1 $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1618658477086) (:by |u0)
              |j $ {} (:text |f1) (:type :leaf) (:at 1618658480301) (:by |u0)
              |r $ {}
                :data $ {}
                :type :expr
                :at 1618658477086
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618658484688) (:by |u0)
                  |j $ {} (:text "|\"calling f1") (:type :leaf) (:at 1618658487989) (:by |u0)
                :type :expr
                :at 1618658483325
                :by |u0
            :type :expr
            :at 1618658477086
            :by |u0
          |try-method $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1622292802864) (:by |u0)
              |j $ {} (:text |try-method) (:type :leaf) (:at 1622292801677) (:by |u0)
              |r $ {}
                :data $ {}
                :type :expr
                :at 1622292801677
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1622292805545) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |.count) (:type :leaf) (:at 1622292806869) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |range) (:type :leaf) (:at 1622292811398) (:by |u0)
                          |j $ {} (:text |11) (:type :leaf) (:at 1622292816464) (:by |u0)
                        :type :expr
                        :at 1622292809130
                        :by |u0
                    :type :expr
                    :at 1622292805914
                    :by |u0
                :type :expr
                :at 1622292803720
                :by |u0
            :type :expr
            :at 1622292801677
            :by |u0
          |call-3 $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1618767957921) (:by |u0)
              |j $ {} (:text |call-3) (:type :leaf) (:at 1618767957921) (:by |u0)
              |r $ {}
                :data $ {}
                  |T $ {} (:text |a) (:type :leaf) (:at 1618767960551) (:by |u0)
                  |j $ {} (:text |b) (:type :leaf) (:at 1618767961787) (:by |u0)
                  |r $ {} (:text |c) (:type :leaf) (:at 1618767962162) (:by |u0)
                :type :expr
                :at 1618767957921
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618767963282) (:by |u0)
                  |j $ {} (:text "|\"a is:") (:type :leaf) (:at 1618767965367) (:by |u0)
                  |r $ {} (:text |a) (:type :leaf) (:at 1618767965784) (:by |u0)
                :type :expr
                :at 1618767962704
                :by |u0
              |x $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618767963282) (:by |u0)
                  |j $ {} (:text "|\"b is:") (:type :leaf) (:at 1618767969236) (:by |u0)
                  |r $ {} (:text |b) (:type :leaf) (:at 1618767970341) (:by |u0)
                :type :expr
                :at 1618767962704
                :by |u0
              |y $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1618767963282) (:by |u0)
                  |j $ {} (:text "|\"c is:") (:type :leaf) (:at 1618767977407) (:by |u0)
                  |r $ {} (:text |c) (:type :leaf) (:at 1618767973639) (:by |u0)
                :type :expr
                :at 1618767962704
                :by |u0
            :type :expr
            :at 1618767957921
            :by |u0
          |rec-sum $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1618723127970) (:by |u0)
              |j $ {} (:text |rec-sum) (:type :leaf) (:at 1618723127970) (:by |u0)
              |r $ {}
                :data $ {}
                  |T $ {} (:text |acc) (:type :leaf) (:at 1618723129611) (:by |u0)
                  |j $ {} (:text |xs) (:type :leaf) (:at 1618723131566) (:by |u0)
                :type :expr
                :at 1618723127970
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |if) (:type :leaf) (:at 1618723136188) (:by |u0)
                  |j $ {}
                    :data $ {}
                      |T $ {} (:text |empty?) (:type :leaf) (:at 1618723138019) (:by |u0)
                      |j $ {} (:text |xs) (:type :leaf) (:at 1618723146569) (:by |u0)
                    :type :expr
                    :at 1618723136714
                    :by |u0
                  |r $ {} (:text |acc) (:type :leaf) (:at 1618723147576) (:by |u0)
                  |v $ {}
                    :data $ {}
                      |T $ {} (:text |recur) (:type :leaf) (:at 1618723151992) (:by |u0)
                      |j $ {}
                        :data $ {}
                          |T $ {} (:text |&+) (:type :leaf) (:at 1618723158533) (:by |u0)
                          |j $ {} (:text |acc) (:type :leaf) (:at 1618723159204) (:by |u0)
                          |r $ {}
                            :data $ {}
                              |T $ {} (:text |nth) (:type :leaf) (:at 1618723268153) (:by |u0)
                              |j $ {} (:text |xs) (:type :leaf) (:at 1618723162178) (:by |u0)
                              |r $ {} (:text |0) (:type :leaf) (:at 1618723268981) (:by |u0)
                            :type :expr
                            :at 1618723160405
                            :by |u0
                        :type :expr
                        :at 1618723153359
                        :by |u0
                      |r $ {}
                        :data $ {}
                          |T $ {} (:text |rest) (:type :leaf) (:at 1618723165126) (:by |u0)
                          |j $ {} (:text |xs) (:type :leaf) (:at 1618723165879) (:by |u0)
                        :type :expr
                        :at 1618723164698
                        :by |u0
                    :type :expr
                    :at 1618723147929
                    :by |u0
                :type :expr
                :at 1618723135708
                :by |u0
            :type :expr
            :at 1618723127970
            :by |u0
          |reload! $ {}
            :data $ {}
              |T $ {} (:text |defn) (:type :leaf) (:at 1619207810174) (:by |u0)
              |j $ {} (:text |reload!) (:type :leaf) (:at 1619207810174) (:by |u0)
              |r $ {}
                :data $ {}
                :type :expr
                :at 1619207810174
                :by |u0
              |v $ {}
                :data $ {}
                  |T $ {} (:text |println) (:type :leaf) (:at 1619766027788) (:by |u0)
                  |j $ {} (:text "|\"reloaded 2") (:type :leaf) (:at 1619766033570) (:by |u0)
                :type :expr
                :at 1619766026889
                :by |u0
              |x $ {}
                :data $ {}
                  |D $ {} (:text |;) (:type :leaf) (:at 1622292791514) (:by |u0)
                  |T $ {} (:text |fib) (:type :leaf) (:at 1619930544016) (:by |u0)
                  |j $ {} (:text |40) (:type :leaf) (:at 1619935071727) (:by |u0)
                :type :expr
                :at 1619930543193
                :by |u0
              |y $ {}
                :data $ {}
                  |T $ {} (:text |try-method) (:type :leaf) (:at 1622292800206) (:by |u0)
                :type :expr
                :at 1622292799913
                :by |u0
            :type :expr
            :at 1619207810174
            :by |u0
        :proc $ {}
          :data $ {}
          :type :expr
          :at 1618539507433
          :by |u0
        :configs $ {}
        :ns $ {}
          :data $ {}
            |T $ {} (:text |ns) (:type :leaf) (:at 1618539507433) (:by |u0)
            |j $ {} (:text |app.main) (:type :leaf) (:at 1618539507433) (:by |u0)
            |r $ {}
              :data $ {}
                |T $ {} (:text |:require) (:type :leaf) (:at 1618661030826) (:by |u0)
                |j $ {}
                  :data $ {}
                    |T $ {} (:text |app.lib) (:type :leaf) (:at 1618661035015) (:by |u0)
                    |j $ {} (:text |:as) (:type :leaf) (:at 1618661039398) (:by |u0)
                    |r $ {} (:text |lib) (:type :leaf) (:at 1618661040510) (:by |u0)
                  :type :expr
                  :at 1618661031081
                  :by |u0
                |r $ {}
                  :data $ {}
                    |T $ {} (:text |app.lib) (:type :leaf) (:at 1618661044709) (:by |u0)
                    |j $ {} (:text |:refer) (:type :leaf) (:at 1618661045794) (:by |u0)
                    |r $ {}
                      :data $ {}
                        |T $ {} (:text |[]) (:type :leaf) (:at 1618661046210) (:by |u0)
                        |j $ {} (:text |f3) (:type :leaf) (:at 1618661047074) (:by |u0)
                      :type :expr
                      :at 1618661046024
                      :by |u0
                  :type :expr
                  :at 1618661042947
                  :by |u0
                |v $ {}
                  :data $ {}
                    |T $ {} (:text |app.macro) (:type :leaf) (:at 1618720199292) (:by |u0)
                    |j $ {} (:text |:refer) (:type :leaf) (:at 1618720200969) (:by |u0)
                    |r $ {}
                      :data $ {}
                        |T $ {} (:text |[]) (:type :leaf) (:at 1618720201399) (:by |u0)
                        |j $ {} (:text |add-num) (:type :leaf) (:at 1618720203059) (:by |u0)
                        |r $ {} (:text |add-by-2) (:type :leaf) (:at 1618740371002) (:by |u0)
                      :type :expr
                      :at 1618720201238
                      :by |u0
                  :type :expr
                  :at 1618720195824
                  :by |u0
              :type :expr
              :at 1618661030124
              :by |u0
          :type :expr
          :at 1618539507433
          :by |u0
  :users $ {}
    |u0 $ {} (:avatar nil) (:name |chen) (:nickname |chen) (:id |u0) (:theme :star-trail) (:password |d41d8cd98f00b204e9800998ecf8427e)
