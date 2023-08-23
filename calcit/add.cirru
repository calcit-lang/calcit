
{} (:package |app)
  :configs $ {} (:init-fn |app.main/main!) (:reload-fn |app.main/reload!)
  :files $ {}
    |app.main $ {}
      :configs $ {}
      :defs $ {}
        |main! $ %{} :CodeEntry
          :code $ quote
            defn main! () $ + 1 2
          :doc |
      :ns $ %{} :CodeEntry
        :code $ quote
          ns app.main $ :require
        :doc |
      :proc $ quote ()
