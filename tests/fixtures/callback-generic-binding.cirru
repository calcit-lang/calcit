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
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () nil
          :schema $ quote $ Fn (-> Nil)
