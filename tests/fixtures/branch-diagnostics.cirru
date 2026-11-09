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
        'Task $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Task (:done Bool)
        'if-mixed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn if-mixed (t) $ if (:done t) 1 |x
          :schema $ quote $ Fn Task (-> Number)
        'cond-mixed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn cond-mixed (n) $ cond
            (> n 1) 1
            true |x
          :schema $ quote $ Fn Number (-> Number)
        'match-mixed $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn match-mixed (o) $ match o
            (:some v) v
            (:none) |x
          :schema $ quote $ Fn (Option Number) (-> Number)
        'cond-no-default $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn cond-no-default (n) $ cond
            (> n 1) 1
            (< n 0) 2
          :schema $ quote $ Fn Number (-> Number)
        'if-ok $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn if-ok (t) $ if (:done t) 1 2
          :schema $ quote $ Fn Task (-> Number)
        'cond-raise $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn cond-raise (n) $ cond
            (> n 1) 1
            true $ raise |unreachable
          :schema $ quote $ Fn Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () nil
          :schema $ quote $ Fn (-> Nil)
