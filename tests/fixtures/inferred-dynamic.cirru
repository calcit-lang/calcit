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
        'open-input $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn open-input (data m)
            let
                copy data
                field $ :k data
                n $ count m
                xs $ map ([] 1 2) $ fn (x) (+ x 1)
              map data $ fn (item) item
          :schema $ quote $ Fn Dynamic (Map Tag Dynamic) (-> Dynamic)
        'untyped $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn untyped ()
            let
                f $ fn (a b) (str a b)
              f 1 2
          :schema $ quote $ Fn (-> Dynamic)
        'typed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn typed (xs)
            let
                total $ foldl xs 0 $ fn (acc x) (+ acc x)
              map xs $ fn (x) (* x total)
          :schema $ quote $ Fn (List Number) (-> (List Number))
        'returned $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn returned (data)
            let
                copy data
              fn (y) (str copy y)
          :schema $ quote $ Fn Dynamic (-> Dynamic)
        'invoked $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn invoked (data)
            (fn (z) (str z)) data
          :schema $ quote $ Fn Dynamic (-> Dynamic)
        'bind-gen $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro bind-gen (value)
            quasiquote $ let
                (~ (turn-symbol |gen)) (~ value)
              str (~ (turn-symbol |gen))
          :schema $ quote $ Macro Syntax
            -> $ Expr Dynamic
        'macro-bound $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn macro-bound (data)
            bind-gen data
          :schema $ quote $ Fn Dynamic (-> Dynamic)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () nil
          :schema $ quote $ Fn (-> Nil)
