
{}
  :users $ {}
    |u0 $ {} (:name |chen) (:id |u0) (:nickname |chen) (:avatar nil) (:password |d41d8cd98f00b204e9800998ecf8427e) (:theme :star-trail)
  :ir $ {} (:package |app)
    :files $ {}
      |app.main $ {}
        :ns $ {} (:type :expr) (:by |u0) (:at 1618539507433)
          :data $ {}
            |T $ {} (:type :leaf) (:by |u0) (:at 1618539507433) (:text |ns)
            |j $ {} (:type :leaf) (:by |u0) (:at 1618539507433) (:text |app.main)
        :defs $ {}
          |main! $ {} (:type :expr) (:by |u0) (:at 1618539520156)
            :data $ {}
              |T $ {} (:type :leaf) (:by |u0) (:at 1618539520156) (:text |defn)
              |j $ {} (:type :leaf) (:by |u0) (:at 1618539520156) (:text |main!)
              |r $ {} (:type :expr) (:by |u0) (:at 1618539520156)
                :data $ {}
              |v $ {} (:type :expr) (:by |u0) (:at 1618539523268)
                :data $ {}
                  |T $ {} (:type :leaf) (:by |u0) (:at 1618539524965) (:text |echo)
                  |j $ {} (:type :leaf) (:by |u0) (:at 1618539525898) (:text "|\"demo")
              |x $ {} (:type :expr) (:by |u0) (:at 1618646117925)
                :data $ {}
                  |T $ {} (:type :leaf) (:by |u0) (:at 1618646119371) (:text |echo)
                  |j $ {} (:type :expr) (:by |u0) (:at 1618646119955)
                    :data $ {}
                      |T $ {} (:type :leaf) (:by |u0) (:at 1618646122999) (:text |&+)
                      |j $ {} (:type :leaf) (:by |u0) (:at 1618646120848) (:text |1)
                      |r $ {} (:type :leaf) (:by |u0) (:at 1618646121081) (:text |2)
        :proc $ {} (:type :expr) (:by |u0) (:at 1618539507433)
          :data $ {}
        :configs $ {}
  :configs $ {} (:reload-fn |app.main/reload!)
    :modules $ []
    :output |src
    :port 6001
    :extension |.cljs
    :local-ui? false
    :init-fn |app.main/main!
    :compact-output? true
    :version |0.0.1
