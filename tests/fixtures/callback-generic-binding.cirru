{}
  :package |demo
  :entries $ {}
    :default $ {} (:init-fn 'demo.main/main!) (:reload-fn 'demo.main/main!) (:mode :js) (:target :browser)
      :modules $ []
  :files $ {}
    'demo.main $ %{} 'FileEntry
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main
      :defs $ {}
        'apply-op $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-op (updater store op) (updater store op)
          :schema $ quote $ Fn (Fn Store Op (-> Store)) Store Op (-> Store) (:generics Op Store)
        'let-bound $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn let-bound () $ let
              updater $ fn (store op) (str store op)
            apply-op updater |base 5
          :schema $ quote $ Fn (-> String)
        'inline $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn inline () $ apply-op (fn (store op) (str store op)) |base 5
          :schema $ quote $ Fn (-> String)
        'inline-bad $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn inline-bad () $ apply-op (fn (store op) (+ store op)) |base 5
          :schema $ quote $ Fn (-> String)
        'alias-bound $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn alias-bound () $ let
              run apply-op
            run (fn (store op) (str store op)) |base 5
          :schema $ quote $ Fn (-> String)
        'param-bound $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn param-bound (run) $ run (fn (store op) (str store op)) |base 5
          :schema $ quote $ Fn (Fn (Fn Store Op (-> Store)) Store Op (-> Store) (:generics Op Store)) (-> String)
        'let-bound-bad $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn let-bound-bad () $ let
              updater $ fn (store op) (+ store op)
            apply-op updater |base 5
          :schema $ quote $ Fn (-> String)
        'apply-rest $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn apply-rest (updater & ops) (foldl ops 0 updater)
          :schema $ quote $ Fn (Fn Number Op (-> Number)) (& Op) (-> Number) (:generics Op)
        'rest-bound $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn rest-bound () $ apply-rest (fn (acc op) (&+ acc (count (.trim op)))) |a |b
          :schema $ quote $ Fn (-> Number)
        'alias-bound-bad $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn alias-bound-bad () $ let
              run apply-op
            run (fn (store op) (+ store op)) |base 5
          :schema $ quote $ Fn (-> String)
        'param-bound-bad $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn param-bound-bad (run) $ run (fn (store op) (+ store op)) |base 5
          :schema $ quote $ Fn (Fn (Fn Store Op (-> Store)) Store Op (-> Store) (:generics Op Store)) (-> String)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () nil
          :schema $ quote $ Fn (-> Nil)
